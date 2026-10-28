// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The C++ backend of [`CodecTarget`].
//!
//! The third spelling of the same walk, and the closest to C's of the
//! three — which makes the places it still differs the interesting ones:
//!
//! - **References, not pointers.** A codec takes `const T &v`, so
//!   [`CodecTarget::member`] is `v.x` and [`CodecTarget::handoff`] hands
//!   the value. C's `&v->x` has no counterpart here and neither does its
//!   array decay: a `T[2][3]` is a `std::array<std::array<T, 3>, 2>`,
//!   which is a value.
//! - **Methods, not free functions.** `runtime/cpp` names them exactly
//!   as the walk asks — `write_u32`, `finish_dheader` — so unlike Java
//!   nothing is re-cased. A codec for a type is a `static` free function
//!   in the probe's translation unit, which C++ does have.
//! - **A `std::vector` knows its own size.** `count_of` is `.size()`
//!   and `make_room` is `.resize(n)`; C's `_length` and `_buffer` have
//!   no counterpart and neither does its allocation guard, because
//!   `resize` throws rather than returning nothing.
//! - **A plain pointer for `@optional`.** An absent member is `nullptr`
//!   and presence is `!= nullptr` (XTypes 1.3 §7.5.1.2.4.2); the codec
//!   reaches the value through [`CodecTarget::behind_indirection`] and a
//!   reader makes it with [`CodecTarget::make_indirection`], the seams C
//!   uses for the same member. There is no flag beside the value, for the
//!   reason `adapter/java.rs` gives for Java: a flag beside the value is
//!   two things that can disagree.
//!
//! What is *not* different is the wire. `runtime/cpp` implements
//! DDS-XTypes 1.3 §7.4 from the clause and calls neither the Rust
//! runtime nor the C one.

use super::{BitPatternSource, CodecTarget, Constant, Handle, Handoff, Routine, Scalar};
use crate::adapter::cpp::{
    cpp_holder, cpp_primitive, cpp_type_path, cpp_type_tag, cpp_value_type, cxx_name,
};
use crate::encode::EncodeError;
use zerodds_ast::{Ast, MemberNode, PrimitiveKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::{Doc, Ident, Literal, Path};
use zerodds_lang_cst::cpp;

/// The C++ backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cpp;

impl Cpp {
    /// A name in the runtime's namespace.
    fn wire(name: &str) -> Path {
        Path::new(&["zerodds", "wire", name])
    }

    /// `zerodds::wire::external_slot(place)` — the runtime function that
    /// fills an `@external` member's `std::shared_ptr` and hands back a
    /// reference to what it now points at. See
    /// `runtime/cpp/include/zerodds/wire.hpp`.
    fn external_slot(place: cpp::Expr) -> cpp::Expr {
        cpp::Expr::Call {
            callee: Box::new(cpp::Expr::Path(Self::wire("external_slot"))),
            args: vec![place],
        }
    }
}

impl CodecTarget for Cpp {
    type Expr = cpp::Expr;
    type Stmt = cpp::Statement;
    type Block = cpp::Block;
    type Type = cpp::Type;
    type Decl = cpp::Decl;

    // -- types --------------------------------------------------------

    /// The fixed-width types, from the materialiser's own table.
    ///
    /// `Index` and `Size` are both `std::size_t` here: C++ has an
    /// unsigned type for each, so the distinction C needs — a `uint32_t`
    /// counter against a `size_t` position — costs nothing to collapse
    /// and a `std::vector::size()` returns the latter.
    fn scalar(kind: Scalar) -> Self::Type {
        let primitive = match kind {
            Scalar::Index | Scalar::Size => {
                return cpp::Type::named(Path::new(&["std", "size_t"]));
            }
            Scalar::Bool => PrimitiveKind::Boolean,
            Scalar::Char => PrimitiveKind::Char,
            Scalar::Int8 => PrimitiveKind::Int8,
            Scalar::Int16 => PrimitiveKind::Int16,
            Scalar::Int32 => PrimitiveKind::Int32,
            Scalar::Int64 => PrimitiveKind::Int64,
            Scalar::UInt8 => PrimitiveKind::UInt8,
            Scalar::UInt16 => PrimitiveKind::UInt16,
            Scalar::UInt32 => PrimitiveKind::UInt32,
            Scalar::UInt64 => PrimitiveKind::UInt64,
            Scalar::Float => PrimitiveKind::Float32,
            Scalar::Double => PrimitiveKind::Float64,
            Scalar::LongDouble => PrimitiveKind::Float128,
        };
        cpp_primitive(primitive)
    }

    fn named(name: &str) -> Self::Type {
        cpp::Type::named(Path::new(&[name]))
    }

    fn nothing() -> Self::Type {
        cpp::Type::Void
    }

    /// The handles are types of the runtime's namespace, not nested in
    /// the writer: `zerodds::wire::DHeader`.
    fn handle(kind: Handle) -> Self::Type {
        cpp::Type::named(Self::wire(match kind {
            Handle::DHeader => "DHeader",
            Handle::EmHeader => "EmHeader",
            Handle::Pid => "Pid",
            // The read direction's four.
            Handle::TryConstruct => "TryConstruct",
            Handle::ReaderMark => "ReaderMark",
            Handle::PidRead => "PidRead",
            Handle::EmHeaderRead => "EmHeaderRead",
        }))
    }

    fn unsigned_holder(bits: u16) -> Self::Type {
        cpp_holder(bits)
    }

    /// A free function of the runtime's namespace.
    ///
    /// `zerodds::wire::no_dheader()` and not `Writer::no_dheader()`,
    /// which is what this used to write. C++ has free functions, so an
    /// operation the walk describes as having no receiver is one; and
    /// the writer's qualification was wrong the moment a *reader* asked
    /// for one — `Writer::pid_is_user_member` names a static member the
    /// writer has no business declaring.
    fn runtime_static(method: &str, args: Vec<Self::Expr>) -> Self::Expr {
        cpp::Expr::Call {
            callee: Box::new(cpp::Expr::Path(Self::wire(method))),
            args,
        }
    }

    /// A policy value, in the runtime's namespace.
    fn constant(which: Constant) -> Self::Expr {
        let (type_, value) = match which {
            Constant::WcharWidth2 => ("WcharWidth", "Two"),
            Constant::WcharWidth4 => ("WcharWidth", "Four"),
            Constant::WcharFixed => ("WcharFraming", "Fixed"),
            Constant::WcharGiopLengthPrefixed => ("WcharFraming", "GiopLengthPrefixed"),
            Constant::WstringUnitsStream => ("WstringUnitOrder", "Stream"),
            Constant::WstringUnitsBigEndianAlways => ("WstringUnitOrder", "BigEndianAlways"),
            Constant::WstringUnitsByteOrderMark => ("WstringUnitOrder", "ByteOrderMark"),
            Constant::WstringUnitsHostUnswapped => ("WstringUnitOrder", "HostUnswapped"),
            Constant::WstringSpecification => ("WstringLengthPolicy", "Specification"),
            Constant::WstringCharacterCount => ("WstringLengthPolicy", "CharacterCount"),
            Constant::WstringCharacterCountTerminated => {
                ("WstringLengthPolicy", "CharacterCountTerminated")
            }
            Constant::WstringSpecificationWideUnits => {
                ("WstringLengthPolicy", "SpecificationWideUnits")
            }
            Constant::WstringRosTypeSupport => ("WstringLengthPolicy", "RosTypeSupport"),
            Constant::WstringOctetsTerminated2 => ("WstringLengthPolicy", "OctetsTerminated2"),
            Constant::WstringOctetsTerminated4 => ("WstringLengthPolicy", "OctetsTerminated4"),
            Constant::WstringCharactersTerminated2 => {
                ("WstringLengthPolicy", "CharactersTerminated2")
            }
            Constant::FixedDeclared => ("FixedDigitCount", "Declared"),
            Constant::FixedValuePrecision => ("FixedDigitCount", "ValuePrecision"),
            Constant::FixedValueStringDigits => ("FixedDigitCount", "ValueStringDigits"),
            Constant::ParameterMember => ("ParameterRole", "Member"),
            Constant::ParameterOptional => ("ParameterRole", "Optional"),
            Constant::TryConstructDiscard => ("TryConstruct", "Discard"),
            Constant::TryConstructTrim => ("TryConstruct", "Trim"),
            Constant::TryConstructUseDefault => ("TryConstruct", "UseDefault"),
            Constant::StatusOk => ("Status", "Ok"),
            Constant::StatusDiscarded => ("Status", "Discarded"),
            Constant::StatusImpossibleLength => ("Status", "ImpossibleLength"),
            Constant::StatusOutOfRange => ("Status", "OutOfRange"),
            Constant::StatusTruncated => ("Status", "Truncated"),
        };
        cpp::Expr::Path(Path::new(&["zerodds", "wire", type_, value]))
    }

    // -- expressions --------------------------------------------------

    fn name(text: &str) -> Self::Expr {
        cpp::Expr::Path(Path::new(&text.split("::").collect::<Vec<_>>()))
    }

    fn int(value: i128) -> Self::Expr {
        cpp::Expr::Literal(Literal::int(value))
    }

    fn literal(value: Literal) -> Self::Expr {
        cpp::Expr::Literal(value)
    }

    /// `strcmp` is the walk's name for a three-way code-unit
    /// comparison and is not a name this column's strings answer to:
    /// the key is a `std::string`, and `a.compare(b)` is
    /// `char_traits<char>::compare` — the same unsigned octet order
    /// C11 §7.24.4 gives `strcmp`, with the same sign convention.
    fn call(callee: &str, args: Vec<Self::Expr>) -> Self::Expr {
        if callee == "strcmp" {
            let mut arguments = args.into_iter();
            if let (Some(left), Some(right)) = (arguments.next(), arguments.next()) {
                return cpp::Expr::MethodCall {
                    receiver: Box::new(left),
                    name: Ident::new("compare"),
                    args: vec![right],
                };
            }
            return cpp::Expr::Call {
                callee: Box::new(Self::name("strcmp")),
                args: Vec::new(),
            };
        }
        cpp::Expr::Call {
            callee: Box::new(Self::name(callee)),
            args,
        }
    }

    fn runtime(receiver: &str, method: &str, args: Vec<Self::Expr>) -> Self::Expr {
        cpp::Expr::MethodCall {
            receiver: Box::new(Self::name(receiver)),
            name: Ident::new(method),
            args,
        }
    }

    /// `runtime/cpp` carries the three HDDS axes: `begin_packed_element`
    /// and `finish_packed_element` on the writer and the reader, the
    /// DHEADER pair for a sequence element's own length, and
    /// `write_wstring_utf8_octets` / `read_wstring_utf8_octets`.
    fn carries_hdds_axes() -> bool {
        true
    }

    /// A reference, so a member is `.` and never `->`.
    fn member(base: &str, field: &str) -> Self::Expr {
        cpp::Expr::Member {
            value: Box::new(Self::name(base)),
            name: Ident::new(field),
        }
    }

    fn whole(base: &str) -> Self::Expr {
        Self::name(base)
    }

    /// `UINT64_C` is C's; C++ writes the suffix.
    ///
    /// −2^63 takes no suffix, because it is not a literal:
    /// `-9223372036854775808LL` is a minus over a literal one past
    /// `LLONG_MAX`. [`zerodds_lang_render::cpp::integer_text`] carries
    /// the clause and the measurement and answers with the subtraction.
    /// The result is a parenthesised expression rather than a literal,
    /// which every site this reaches admits: the walk puts it inside a
    /// `static_cast` in an `if` condition and never where C++ requires
    /// a token — the generated union reader is a chain of comparisons
    /// and carries no `case` label.
    fn wide_int(value: i128) -> Self::Expr {
        let text = if value > i128::from(i64::MAX) {
            format!("{value}ULL")
        } else if value == i128::from(i64::MIN) {
            zerodds_lang_render::cpp::integer_text(value)
        } else if value > i128::from(i32::MAX) || value < i128::from(i32::MIN) {
            format!("{value}LL")
        } else {
            format!("{value}")
        };
        cpp::Expr::Literal(Literal::Int(text))
    }

    fn field_of(value: Self::Expr, field: &str) -> Self::Expr {
        cpp::Expr::Member {
            value: Box::new(value),
            name: Ident::new(field),
        }
    }

    /// The storage behind the accessor of a case (`adapter::cpp::union_storage`):
    /// the codec reads and writes the storage and not the accessor, whose
    /// setter selects the case.
    fn case_member(base: &str, field: &str) -> Self::Expr {
        Self::member(base, &crate::adapter::cpp::union_storage(field))
    }

    fn discriminant(base: &str) -> Self::Expr {
        Self::member(base, crate::adapter::cpp::UNION_DISCRIMINATOR)
    }

    fn index(value: Self::Expr, at: Self::Expr) -> Self::Expr {
        cpp::Expr::Index {
            value: Box::new(value),
            index: Box::new(at),
        }
    }

    /// A `std::vector` and a `std::array` both answer `.size()`, so the
    /// bound the materialiser fixed is not needed and not used.
    fn count_of(base: &str, _fixed: Option<u64>) -> Self::Expr {
        cpp::Expr::MethodCall {
            receiver: Box::new(Self::name(base)),
            name: Ident::new("size"),
            args: Vec::new(),
        }
    }

    fn binary(op: &'static str, lhs: Self::Expr, rhs: Self::Expr) -> Self::Expr {
        cpp::Expr::Binary {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        }
    }

    /// The tree has no unary node; `!a` is `a == false` and `-a` is
    /// `0 - a`, which is what both mean.
    /// `!a` as `a == false`, and every other operator as itself.
    ///
    /// The negation is spelled out because `!` on a value the walk
    /// hands over is `operator!` on whatever type it has, and `== false`
    /// says the same thing for a `bool` without depending on that. The
    /// rest go through [`cpp::Expr::Unary`], which is what a complement
    /// needs: the form this used before wrote `0 ~ a`, which no
    /// compiler reads.
    fn unary(op: &'static str, value: Self::Expr) -> Self::Expr {
        match op {
            "!" => cpp::Expr::Binary {
                op: "==",
                lhs: Box::new(value),
                rhs: Box::new(cpp::Expr::Literal(Literal::Bool(false))),
            },
            _ => cpp::Expr::Unary {
                op,
                value: Box::new(value),
            },
        }
    }

    /// The bracket is written, because the walk asked for it.
    ///
    /// This returned the expression unchanged, on the reading that "the
    /// renderer brackets by precedence". It does not — `zerodds-lang-cst`
    /// says so in as many words, on `Expr::Paren`: "this tree holds no
    /// operator precedence, so a renderer walking it cannot know where a
    /// bracket is needed. A builder that needs one says so."
    ///
    /// What that cost: a bitmask's reader asks for
    /// `paren(zd_bits & ~UINT64_C(131)) != 0` and got
    /// `zd_bits & ~UINT64_C(131) != 0`, which C++ groups as
    /// `zd_bits & (~131 != 0)` — a different number, and one that would
    /// have accepted every undeclared bit. `-Wparentheses -Werror` made
    /// it a build failure rather than a wrong value, which is the only
    /// reason it was found at the compiler and not in a byte comparison.
    fn paren(value: Self::Expr) -> Self::Expr {
        cpp::Expr::Paren(Box::new(value))
    }

    fn cast(type_: Self::Type, value: Self::Expr) -> Self::Expr {
        cpp::Expr::Cast {
            kind: cpp::CastKind::Static,
            type_: Box::new(type_),
            value: Box::new(value),
        }
    }

    /// A reference takes the value either way: there is no address to
    /// take and no array to decay.
    fn handoff(value: Self::Expr, _form: Handoff) -> Self::Expr {
        value
    }

    /// `write_char` takes a `char32_t`, so the code point is a cast to
    /// it and the runtime keeps what the width allows.
    fn code_point(value: Self::Expr, _narrow: bool) -> Self::Expr {
        cpp::Expr::Cast {
            kind: cpp::CastKind::Static,
            type_: Box::new(cpp::Type::named(Path::new(&["char32_t"]))),
            value: Box::new(value),
        }
    }

    /// The integer form is two `static_cast`s — through the unsigned
    /// type of the source's width first, which is what masks a negative
    /// value to its two's-complement octets instead of sign-extending
    /// it — and the float forms are the runtime's `bit_pattern_f32` and
    /// `bit_pattern_f64`, because C++ has no expression that reads a
    /// float's bits without `std::memcpy`.
    fn bit_pattern(source: BitPatternSource, value: Self::Expr) -> Self::Expr {
        let through = |narrow: &str, value: cpp::Expr| cpp::Expr::Cast {
            kind: cpp::CastKind::Static,
            type_: Box::new(cpp::Type::named(Path::new(&["std", "uint64_t"]))),
            value: Box::new(cpp::Expr::Cast {
                kind: cpp::CastKind::Static,
                type_: Box::new(cpp::Type::named(Path::new(&["std", narrow]))),
                value: Box::new(value),
            }),
        };
        match source {
            BitPatternSource::Int { octets } => {
                let narrow = match octets {
                    1 => "uint8_t",
                    2 => "uint16_t",
                    4 => "uint32_t",
                    _ => "uint64_t",
                };
                through(narrow, value)
            }
            BitPatternSource::Bool => cpp::Expr::Cast {
                kind: cpp::CastKind::Static,
                type_: Box::new(cpp::Type::named(Path::new(&["std", "uint64_t"]))),
                value: Box::new(value),
            },
            BitPatternSource::Float32 => Self::runtime_static("bit_pattern_f32", vec![value]),
            BitPatternSource::Float64 => Self::runtime_static("bit_pattern_f64", vec![value]),
        }
    }

    // -- statements ---------------------------------------------------

    fn declare(type_: Self::Type, name: &str, value: Option<Self::Expr>) -> Self::Stmt {
        cpp::Statement::Declare {
            type_,
            name: Ident::new(name),
            value,
        }
    }

    fn perform(value: Self::Expr) -> Self::Stmt {
        cpp::Statement::Expr(value)
    }

    fn give_back(value: Option<Self::Expr>) -> Self::Stmt {
        cpp::Statement::Return(value)
    }

    fn when(
        condition: Self::Expr,
        then: Self::Block,
        otherwise: Option<Self::Block>,
    ) -> Self::Stmt {
        cpp::Statement::If {
            condition,
            then,
            otherwise,
        }
    }

    /// `for (std::size_t i = 0; i < bound; i = i + 1)`, the step written
    /// out for the reason C's is: the tree has no increment node, and
    /// one spelling across the backends keeps two columns' generated
    /// source comparable by eye when their octets disagree.
    fn count_up(counter: &str, bound: Self::Expr, body: Self::Block) -> Self::Stmt {
        cpp::Statement::For {
            init: Box::new(cpp::Statement::Declare {
                type_: cpp::Type::named(Path::new(&["std", "size_t"])),
                name: Ident::new(counter),
                value: Some(cpp::Expr::Literal(Literal::int(0))),
            }),
            condition: cpp::Expr::Binary {
                op: "<",
                lhs: Box::new(Self::name(counter)),
                rhs: Box::new(bound),
            },
            step: cpp::Expr::Binary {
                op: "=",
                lhs: Box::new(Self::name(counter)),
                rhs: Box::new(cpp::Expr::Binary {
                    op: "+",
                    lhs: Box::new(Self::name(counter)),
                    rhs: Box::new(cpp::Expr::Literal(Literal::int(1))),
                }),
            },
            body,
        }
    }

    fn until(condition: Self::Expr, body: Self::Block) -> Self::Stmt {
        cpp::Statement::While { condition, body }
    }

    fn leave() -> Self::Stmt {
        cpp::Statement::Break
    }

    fn note(doc: Doc) -> Self::Stmt {
        cpp::Statement::Comment(doc)
    }

    /// `v.x != nullptr`. XTypes 1.3 §7.5.1.2.4.2: "A NULL pointer shall
    /// indicate an omitted value."
    fn present(base: &str, field: &str) -> Self::Expr {
        cpp::Expr::Binary {
            op: "!=",
            lhs: Box::new(Self::member(base, field)),
            rhs: Box::new(Self::name("nullptr")),
        }
    }

    /// Nothing on the way in — [`Cpp::make_indirection`] makes the object
    /// — and `zerodds::wire::destroy(v.x);` on the way out, which deletes
    /// what the member held and leaves it NULL.
    fn set_present(base: &str, field: &str, present: bool) -> Vec<Self::Stmt> {
        if present {
            return Vec::new();
        }
        vec![cpp::Statement::Expr(cpp::Expr::Call {
            callee: Box::new(cpp::Expr::Path(Self::wire("destroy"))),
            args: vec![Self::member(base, field)],
        })]
    }

    /// An `@optional` member is a plain pointer (§7.5.1.2.4.2).
    fn optionals_are_pointers() -> bool {
        true
    }

    /// A `std::string` is a value, so an `@optional string` is a
    /// `std::string *` like any other.
    fn value_is_pointer(_ast: &Ast, _id: Id<TypeNode>) -> bool {
        false
    }

    /// `if (!(v.x != nullptr)) { throw zerodds::wire::UnsetExternal(...); }`
    fn refuse_unset_external(present: Self::Expr) -> Vec<Self::Stmt> {
        vec![cpp::Statement::If {
            condition: cpp::Expr::Unary {
                op: "!",
                value: Box::new(cpp::Expr::Paren(Box::new(present.clone()))),
            },
            then: cpp::Block {
                statements: vec![cpp::Statement::Throw(cpp::Expr::Construct {
                    type_: Box::new(cpp::Type::named(Self::wire("UnsetExternal"))),
                    args: vec![
                        cpp::Expr::MethodCall {
                            receiver: Box::new(Self::name("w")),
                            name: Ident::new("position"),
                            args: Vec::new(),
                        },
                        cpp::Expr::Literal(Literal::string(&cpp_text_of(&present))),
                    ],
                    braced: false,
                })],
            },
            otherwise: None,
        }]
    }

    /// The vector is the elements.
    /// `.value()` and not `*`: the two are the same inside a guarded
    /// branch and the first says which one it is.
    /// A `std::pair`, so `first` and `second`.
    fn entry_half(entry: Self::Expr, key: bool) -> Self::Expr {
        cpp::Expr::Member {
            value: Box::new(entry),
            name: Ident::new(if key { "first" } else { "second" }),
        }
    }

    /// The member, the pointer; [`Cpp::behind_indirection`] dereferences it.
    fn optional_value(_ast: &Ast, _id: Id<TypeNode>, base: &str, field: &str) -> Self::Expr {
        Self::member(base, field)
    }

    /// `*place` — the object an optional pointer points at.
    fn behind_indirection(_ast: &Ast, _id: Id<TypeNode>, place: Self::Expr) -> Self::Expr {
        cpp::Expr::Deref(Box::new(place))
    }

    /// `zerodds::wire::pointer_slot(place)` — the pointer made to point at
    /// a new object when it points nowhere, and the object.
    fn make_indirection(
        _ast: &Ast,
        _id: Id<TypeNode>,
        place: Self::Expr,
    ) -> (Vec<Self::Stmt>, Self::Expr) {
        (
            Vec::new(),
            cpp::Expr::Call {
                callee: Box::new(cpp::Expr::Path(Self::wire("pointer_slot"))),
                args: vec![place],
            },
        )
    }

    /// `true`: an `@external` member is a `std::shared_ptr`, and a member on a
    /// cycle needs the same: `std::optional<T>` and a plain `T` both want
    /// `T` complete.
    /// [`crate::adapter::held_externally`] is the rule the adapter and
    /// the defaulter ask as well.
    fn holds_a_cycle_externally() -> bool {
        true
    }

    /// `*v.x` — the shared pointer dereferenced, for `@external` with
    /// and without `@optional` alike: the member's codec takes the
    /// value, and `crates/lang-rules/src/adapter/cpp.rs` gives an
    /// `@external` member one indirection and no `std::optional` around
    /// it.
    fn external_value(
        _ast: &Ast,
        _id: Id<TypeNode>,
        base: &str,
        field: &str,
        _optional: bool,
    ) -> Self::Expr {
        cpp::Expr::Unary {
            op: "*",
            value: Box::new(Self::member(base, field)),
        }
    }

    /// `zerodds::wire::external_slot(v.x)` — the slot filled and the
    /// place handed back, in one expression.
    ///
    /// The counterpart of [`Cpp::optional_destination`]'s `emplace`: a
    /// pointer member has nothing to write into until it points at
    /// something. A function of the runtime and not
    /// `std::make_shared<T>()` written here, because this seam is handed
    /// the member and not its type.
    fn external_destination(
        base: &str,
        field: &str,
        _optional: bool,
    ) -> (Vec<Self::Stmt>, Self::Expr) {
        (Vec::new(), Self::external_slot(Self::member(base, field)))
    }

    /// `*v.x` for a branch too: a union arm's `@external` field is the
    /// same `std::shared_ptr<T>` a structure member is.
    fn external_case_value(_ast: &Ast, _id: Id<TypeNode>, base: &str, field: &str) -> Self::Expr {
        Self::external_value(
            _ast,
            _id,
            base,
            &crate::adapter::cpp::union_storage(field),
            false,
        )
    }

    /// The same allocation and the same place as
    /// [`Cpp::external_destination`].
    fn external_case_destination(base: &str, field: &str) -> (Vec<Self::Stmt>, Self::Expr) {
        Self::external_destination(base, &crate::adapter::cpp::union_storage(field), false)
    }

    /// `v.x != nullptr`: the pointer is the absence, with no
    /// `std::optional` around it to carry a second one.
    fn external_present(base: &str, field: &str) -> Self::Expr {
        cpp::Expr::Binary {
            op: "!=",
            lhs: Box::new(Self::member(base, field)),
            rhs: Box::new(Self::name("nullptr")),
        }
    }

    /// Nothing on the way in — [`Cpp::external_destination`] allocates —
    /// and `v.x = nullptr;` on the way out.
    fn external_set_present(base: &str, field: &str, present: bool) -> Vec<Self::Stmt> {
        if present {
            return Vec::new();
        }
        vec![cpp::Statement::Expr(cpp::Expr::Binary {
            op: "=",
            lhs: Box::new(Self::member(base, field)),
            rhs: Box::new(Self::name("nullptr")),
        })]
    }

    /// `v[i]`, except for the one container C++ specialises.
    ///
    /// `std::vector<bool>` packs its elements into bits — [vector.bool] —
    /// and `operator[]` hands back a proxy object rather than a
    /// `bool &`. A generated decoder takes its destination by reference,
    /// so there is nothing for it to point at, and the call does not
    /// compile. Read into a local and assign it back: one statement
    /// either side, and only where the element really is `bool`.
    ///
    /// Not solved by changing what a `sequence<boolean>` maps to. The
    /// element codec is named by the IDL kind — `zd_dec_p_boolean` —
    /// takes a `bool &`, and is the same function whether the value is a
    /// member, an array element or a map's value, so a container holding
    /// anything but `bool` would not match it either. The proxy is the
    /// container's, so the work-around belongs at the container.
    fn sequence_element(
        base: &str,
        index: Self::Expr,
        element: Self::Type,
    ) -> (Vec<Self::Stmt>, Self::Expr, Vec<Self::Stmt>) {
        let slot = cpp::Expr::Index {
            value: Box::new(Self::elements(base)),
            index: Box::new(index),
        };
        if element != cpp::Type::Primitive(cpp::Primitive::Bool) {
            return (Vec::new(), slot, Vec::new());
        }
        let local = "zd_bit";
        (
            vec![cpp::Statement::Declare {
                type_: cpp::Type::Primitive(cpp::Primitive::Bool),
                name: Ident::new(local),
                value: Some(cpp::Expr::Literal(Literal::Bool(false))),
            }],
            Self::name(local),
            vec![cpp::Statement::Expr(cpp::Expr::Binary {
                op: "=",
                lhs: Box::new(slot),
                rhs: Box::new(Self::name(local)),
            })],
        )
    }

    /// `v.resize(count);`, which is `make_room` again in the shortening
    /// direction. A `std::vector` owns its elements, so the ones past
    /// the bound are destroyed rather than left where C leaves them.
    fn keep_first(base: &str, count: Self::Expr) -> Vec<Self::Stmt> {
        vec![cpp::Statement::Expr(cpp::Expr::MethodCall {
            receiver: Box::new(Self::name(base)),
            name: Ident::new("resize"),
            args: vec![count],
        })]
    }

    /// The member: [`Cpp::make_indirection`] makes it point somewhere.
    fn optional_destination(base: &str, field: &str) -> (Vec<Self::Stmt>, Self::Expr) {
        (Vec::new(), Self::member(base, field))
    }

    fn elements(base: &str) -> Self::Expr {
        Self::name(base)
    }

    /// `w.write_octets(v.data(), n)`: `std::vector<std::uint8_t>` and
    /// `std::array<std::uint8_t, N>` both hold their octets contiguously.
    fn bulk_octets_write(base: &str, count: Self::Expr) -> Option<Self::Stmt> {
        Some(Self::perform(Self::runtime(
            "w",
            "write_octets",
            vec![data_of(base), count],
        )))
    }

    /// `r.read_octets(v.data(), n)`, after `make_room` has resized `v`.
    fn bulk_octets_read(base: &str, count: Self::Expr) -> Option<Self::Stmt> {
        Some(Self::perform(Self::runtime(
            "r",
            "read_octets",
            vec![data_of(base), count],
        )))
    }

    /// One statement: `resize` throws rather than returning nothing, so
    /// there is no guard and the fill follows unconditionally.
    fn make_room(
        base: &str,
        _element: Self::Type,
        count: Self::Expr,
        // C's slot is storage the moment the buffer is there, so
        // whether the fill assigns it or writes into it makes no
        // difference to what has to be allocated.
        _assigned_whole: bool,
        fill: Vec<Self::Stmt>,
    ) -> Vec<Self::Stmt> {
        let mut out = vec![cpp::Statement::Expr(cpp::Expr::MethodCall {
            receiver: Box::new(Self::name(base)),
            name: Ident::new("resize"),
            args: vec![count],
        })];
        out.extend(fill);
        out
    }

    fn absent() -> Self::Expr {
        cpp::Expr::Path(Path::new(&["nullptr"]))
    }

    /// Nothing to check. Every value this column's reader materialises
    /// is a `std::string` or a `std::vector`, which own their storage
    /// and are never absent; the read either produced one or threw.
    fn materialised(_value: Self::Expr) -> Option<Self::Expr> {
        None
    }

    // -- assembly -----------------------------------------------------

    fn block(statements: Vec<Self::Stmt>) -> Self::Block {
        cpp::Block { statements }
    }

    fn into_statements(block: Self::Block) -> Vec<Self::Stmt> {
        block.statements
    }

    /// `static void zd_enc_T(zerodds::wire::Writer &w, const T &v)`.
    ///
    /// A prototype, unlike Java's, is emitted: C++ resolves a call to a
    /// free function by what is declared above it, so a recursive or
    /// mutually recursive type needs the declaration ahead of the body
    /// exactly as C does.
    fn routine(spec: Routine<Self>) -> Option<Self::Decl> {
        let stream = cpp::Param {
            type_: cpp::Type::Ref(Box::new(cpp::Type::named(Self::wire(if spec.writing {
                "Writer"
            } else {
                "Reader"
            })))),
            name: Some(Ident::new(if spec.writing { "w" } else { "r" })),
            default: None,
        };
        let mut params = vec![
            stream,
            cpp::Param {
                // `const` in the written direction and not in the read
                // one, the same distinction C makes with its pointer.
                type_: cpp::Type::Ref(Box::new(if spec.writing {
                    cpp::Type::Const(Box::new(spec.sample))
                } else {
                    spec.sample
                })),
                name: Some(Ident::new("v")),
                default: None,
            },
        ];
        params.extend(spec.extra.into_iter().map(|(type_, name)| cpp::Param {
            type_,
            name: Some(Ident::new(&name)),
            default: None,
        }));
        // `(void)v;` and `(void)w;` ahead of the body, for the reason
        // the C reader writes `(void)zd_tc;`: an aggregate with no
        // members has nothing to write, and `-Wunused-parameter
        // -Werror` fails the build on the one case that is legal C++
        // and empty. Emitted unconditionally rather than from a guess
        // about which bodies touch which parameter.
        //
        // The stream was left out of that until an empty `@appendable`
        // structure was read under XCDR1, where the type takes no
        // delimiter either — so the body was empty in both directions
        // and neither parameter was touched. The C column cannot reach
        // it: C23 §6.7.2.1p8 leaves a structure with no named member
        // undefined and the C materialiser refuses one.
        let stream = if spec.writing { "w" } else { "r" };
        let body = spec.body.map(|block| {
            let mut statements = vec![
                cpp::Statement::Expr(cpp::Expr::Cast {
                    kind: cpp::CastKind::Static,
                    type_: Box::new(cpp::Type::Void),
                    value: Box::new(Self::name(stream)),
                }),
                cpp::Statement::Expr(cpp::Expr::Cast {
                    kind: cpp::CastKind::Static,
                    type_: Box::new(cpp::Type::Void),
                    value: Box::new(Self::name("v")),
                }),
            ];
            statements.extend(Self::into_statements(block));
            Self::block(statements)
        });
        Some(cpp::Decl::Function(cpp::Function {
            doc: spec.doc,
            returns: cpp::Type::Void,
            name: Ident::new(&spec.name),
            params,
            is_inline: false,
            is_static: true,
            is_constexpr: false,
            is_noexcept: false,
            // `CodecTarget::routine`'s `tolerate_unused`, which
            // `crate::emit::encode::maybe_unused` sets for the one codec
            // that may have no caller: a `wchar` whose element form is
            // written inline under TAO's framing. g++ refused the probe
            // with `-Werror=unused-function` on `zd_enc_p_wchar` in 48
            // matrix cells (TAO dialect, every case with a `wchar` run).
            is_maybe_unused: spec.tolerate_unused,
            // A body absent is a declaration and not an empty one: C++
            // resolves a call to a free function by what is declared
            // above it.
            body: body.map_or(cpp::FunctionBody::None, cpp::FunctionBody::Block),
        }))
    }

    // -- the materialiser's boundary ----------------------------------

    fn value_type(ast: &Ast, id: Id<TypeNode>) -> Self::Type {
        cpp_value_type(ast, id, "codec").unwrap_or_else(|_| {
            cpp::Type::named(Path::new(
                &[format!("UnresolvedType{}", id.slot()).as_str()],
            ))
        })
    }

    fn type_name(ast: &Ast, id: Id<TypeNode>) -> String {
        cpp_type_tag(ast, id)
    }

    fn escape(name: &str) -> String {
        cxx_name(name)
    }

    fn map_entry_name(ast: &Ast, id: Id<TypeNode>) -> String {
        cpp_type_tag(ast, id)
    }

    /// `std::pair<K, V>` — the type the materialiser spells the entry
    /// as. The tag `map_entry_name` answers is a *name* for generated
    /// identifiers (`zd_dec_map_string_long`), not a declared type, so
    /// the default's `named(tag)` declared the scratch entry with an
    /// identifier no compiler finds.
    fn map_entry_type(ast: &Ast, id: Id<TypeNode>) -> Self::Type {
        let Ok(zerodds_ast::TypeKind::Map { key, value, .. }) =
            ast.type_(id).map(|node| node.kind.clone())
        else {
            return Self::named(&Self::map_entry_name(ast, id));
        };
        let at = cpp_type_tag(ast, id);
        let half = |half: Option<Id<TypeNode>>| {
            half.and_then(|id| cpp_value_type(ast, id, &at).ok())
                .unwrap_or(cpp::Type::Void)
        };
        cpp::Type::template(
            Path::new(&["std", "pair"]),
            vec![half(key.resolved()), half(value.resolved())],
        )
    }

    /// C++ has namespaces, so a module needs no prefix in a name.
    fn module_prefix(_ast: &Ast, _id: Id<TypeNode>) -> String {
        String::new()
    }

    /// `::a::b::Colour::red`.
    ///
    /// The enumeration's own name is in the middle, which is what makes
    /// this a seam rather than a prefix and a bare identifier:
    /// `adapter/cpp.rs` declares a **scoped** enumeration, so an
    /// enumerator is a member of the type. Its reason is written there —
    /// an unscoped `enum` would put every enumerator of every
    /// enumeration of a module in one namespace, where two IDL-legal
    /// names collide.
    fn enumerator_constant(ast: &Ast, id: Id<TypeNode>, name: &str) -> Self::Expr {
        Self::name(&enumerator_constant_name(ast, id, name))
    }

    /// A call to the generated defaulter, as the C column does it.
    ///
    /// Not `place = T{}`. Value-initialisation is not Table 9 — an
    /// enumeration whose first literal is not zero, a union's selected
    /// branch and a member's own `@default` are three rows it gets wrong
    /// — and two columns answering the same table by different
    /// mechanisms is a divergence no byte comparison shows, because both
    /// produce *a* value. [`crate::default_cpp`] carries the whole
    /// argument.
    ///
    /// # Errors
    /// Never here. The defaulter is a call, and whether the type has one
    /// is settled where the function is built —
    /// [`crate::default_cpp::cpp_defaulters_called_by`] reads the calls
    /// this emitted and refuses there, by name.
    fn default_into(
        ast: &Ast,
        id: Id<TypeNode>,
        place: Self::Expr,
    ) -> Result<Self::Stmt, EncodeError> {
        Ok(crate::default_cpp::default_into(ast, id, place))
    }

    /// # Errors
    /// [`EncodeError::NoWriter`] where a `@default` names a value C++
    /// cannot spell — a `long double` literal is the one the corpus
    /// reaches.
    fn member_default(
        ast: &Ast,
        node: &MemberNode,
        type_: Id<TypeNode>,
        place: Self::Expr,
    ) -> Result<Vec<Self::Stmt>, EncodeError> {
        crate::default_cpp::member_default(ast, node, type_, place)
    }
}

/// The scoped name of one enumerator, as text.
///
/// The enumeration's own name is in the middle, which is what makes
/// this more than a prefix and a bare identifier: `adapter/cpp.rs`
/// declares a **scoped** enumeration, so an enumerator is a member of
/// the type. `default_cpp` builds a path from this, and assembling the
/// name a second time in that file would be a second place the C++
/// spelling is decided.
pub fn enumerator_constant_name(ast: &Ast, id: Id<TypeNode>, name: &str) -> String {
    let mut path = cpp_type_path(ast, id)
        .segments()
        .iter()
        .map(|segment| segment.as_str().to_string())
        .collect::<Vec<_>>()
        .join("::");
    path.push_str("::");
    path.push_str(&cxx_name(name));
    format!("::{path}")
}

/// `base.data()`: the first element of a contiguous collection.
fn data_of(base: &str) -> cpp::Expr {
    cpp::Expr::MethodCall {
        receiver: Box::new(Cpp::name(base)),
        name: Ident::new("data"),
        args: Vec::new(),
    }
}

/// The member's name in a presence test the walk built: `v.x != nullptr`
/// gives `x`. Only for the text of a diagnostic.
fn cpp_text_of(present: &cpp::Expr) -> String {
    if let cpp::Expr::Binary { lhs, .. } = present {
        if let cpp::Expr::Member { name, .. } = lhs.as_ref() {
            return name.as_str().to_string();
        }
    }
    String::new()
}
