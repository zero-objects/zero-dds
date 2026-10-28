// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The C backend of [`CodecTarget`].
//!
//! Every method here builds what [`crate::encode_c`] built by hand
//! before the walk was made generic, node for node. That is the
//! contract this backend is held to: the C column's octets are recorded
//! over 17 220 slots, and a backend that produced tidier C would be
//! reporting different bytes for the same layout key.
//!
//! ## What C makes visible here
//!
//! - **A pointer parameter.** A codec takes `const T *v` and reaches its
//!   members with `->`. [`CodecTarget::member`] is that arrow, and the
//!   targets that hand a reference spell the same call `.`.
//! - **An array parameter decays.** C adjusts an array parameter to a
//!   pointer to its first element, so a call site passes `v->grid` for
//!   an array and `&v->x` for everything else — [`Handoff`].
//! - **One namespace.** `runtime/c` exports `zdw_*` for both directions;
//!   the reader's functions are `zdw_read_*` and not `zdr_*`. The
//!   receiver local is what tells them apart at a call site.

use super::{BitPatternSource, CodecTarget, Constant, Handle, Handoff, Routine, Scalar};
use crate::adapter::c::{c_codec_tag, c_holder, c_map_entry_name, c_value_type, module_prefix};
use crate::encode::EncodeError;
use zerodds_ast::{Ast, MemberNode, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_cst::c;
use zerodds_lang_cst::common::{Doc, Ident, Literal};

/// The C backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct C;

impl C {
    /// The runtime's prefix, which both directions share.
    const PREFIX: &'static str = "zdw_";
}

impl CodecTarget for C {
    type Expr = c::Expr;
    type Stmt = c::Statement;
    type Block = c::Block;
    type Type = c::Type;
    type Decl = c::Decl;

    // -- types --------------------------------------------------------

    fn scalar(kind: Scalar) -> Self::Type {
        c::Type::Primitive(match kind {
            Scalar::Index => c::Primitive::UInt32,
            Scalar::Size => c::Primitive::Size,
            Scalar::Bool => c::Primitive::Bool,
            Scalar::Char => c::Primitive::Char,
            Scalar::Int8 => c::Primitive::Int8,
            Scalar::Int16 => c::Primitive::Int16,
            Scalar::Int32 => c::Primitive::Int32,
            Scalar::Int64 => c::Primitive::Int64,
            Scalar::UInt8 => c::Primitive::UInt8,
            Scalar::UInt16 => c::Primitive::UInt16,
            Scalar::UInt32 => c::Primitive::UInt32,
            Scalar::UInt64 => c::Primitive::UInt64,
            Scalar::Float => c::Primitive::Float,
            Scalar::Double => c::Primitive::Double,
            Scalar::LongDouble => c::Primitive::LongDouble,
        })
    }

    fn named(name: &str) -> Self::Type {
        c::Type::Named(Ident::new(name))
    }

    fn nothing() -> Self::Type {
        c::Type::Void
    }

    /// The runtime's own small structs, in its single namespace.
    fn handle(kind: Handle) -> Self::Type {
        c::Type::Named(Ident::new(match kind {
            Handle::DHeader => "zdw_dheader",
            Handle::EmHeader => "zdw_emheader",
            Handle::Pid => "zdw_pid",
            Handle::TryConstruct => "zdw_try_construct",
            Handle::ReaderMark => "zdw_reader_mark",
            Handle::PidRead => "zdw_pid_read",
            Handle::EmHeaderRead => "zdw_emheader_read",
        }))
    }

    fn unsigned_holder(bits: u16) -> Self::Type {
        c::Type::Primitive(c_holder(bits))
    }

    /// `(uint32_t)(uint8_t)v` for a `char` and `(uint32_t)v` for a
    /// `wchar`, which is what the recorded octets were produced with.
    fn code_point(value: Self::Expr, narrow: bool) -> Self::Expr {
        let inner = if narrow {
            c::Expr::Cast {
                type_: Box::new(c::Type::Primitive(c::Primitive::UInt8)),
                value: Box::new(value),
            }
        } else {
            value
        };
        c::Expr::Cast {
            type_: Box::new(c::Type::Primitive(c::Primitive::UInt32)),
            value: Box::new(inner),
        }
    }

    /// `runtime/c` publishes these as macros in one namespace.
    fn constant(which: Constant) -> Self::Expr {
        c::Expr::Name(Ident::new(match which {
            Constant::WcharWidth2 => "ZDW_WCHAR_WIDTH_2",
            Constant::WcharWidth4 => "ZDW_WCHAR_WIDTH_4",
            Constant::WcharFixed => "ZDW_WCHAR_FIXED",
            Constant::WcharGiopLengthPrefixed => "ZDW_WCHAR_GIOP_LENGTH_PREFIXED",
            Constant::WstringUnitsStream => "ZDW_WSTRING_UNITS_STREAM",
            Constant::WstringUnitsBigEndianAlways => "ZDW_WSTRING_UNITS_BIG_ENDIAN_ALWAYS",
            Constant::WstringUnitsByteOrderMark => "ZDW_WSTRING_UNITS_BYTE_ORDER_MARK",
            Constant::WstringUnitsHostUnswapped => "ZDW_WSTRING_UNITS_HOST_UNSWAPPED",
            Constant::WstringSpecification => "ZDW_WSTRING_SPECIFICATION",
            Constant::WstringCharacterCount => "ZDW_WSTRING_CHARACTER_COUNT",
            Constant::WstringCharacterCountTerminated => "ZDW_WSTRING_CHARACTER_COUNT_TERMINATED",
            Constant::WstringSpecificationWideUnits => "ZDW_WSTRING_SPECIFICATION_WIDE_UNITS",
            Constant::WstringRosTypeSupport => "ZDW_WSTRING_ROS_TYPE_SUPPORT",
            Constant::WstringOctetsTerminated2 => "ZDW_WSTRING_OCTETS_TERMINATED_2",
            Constant::WstringOctetsTerminated4 => "ZDW_WSTRING_OCTETS_TERMINATED_4",
            Constant::WstringCharactersTerminated2 => "ZDW_WSTRING_CHARACTERS_TERMINATED_2",
            Constant::FixedDeclared => "ZDW_FIXED_DECLARED",
            Constant::FixedValuePrecision => "ZDW_FIXED_VALUE_PRECISION",
            Constant::FixedValueStringDigits => "ZDW_FIXED_VALUE_STRING_DIGITS",
            Constant::ParameterMember => "ZDW_PARAM_MEMBER",
            Constant::ParameterOptional => "ZDW_PARAM_OPTIONAL",
            Constant::TryConstructDiscard => "ZDW_TC_DISCARD",
            Constant::TryConstructTrim => "ZDW_TC_TRIM",
            Constant::TryConstructUseDefault => "ZDW_TC_USE_DEFAULT",
            Constant::StatusOk => "ZDW_OK",
            Constant::StatusDiscarded => "ZDW_DISCARDED",
            Constant::StatusImpossibleLength => "ZDW_IMPOSSIBLE_LENGTH",
            Constant::StatusOutOfRange => "ZDW_OUT_OF_RANGE",
            Constant::StatusTruncated => "ZDW_TRUNCATED",
        }))
    }

    /// The walk's `Int64` cast turned around: `zdw_bit_pattern_int`
    /// takes `uint64_t` (the pattern is storage bits, not a number),
    /// and handing it the walk's `(int64_t)` is the implicit
    /// signedness conversion `-Wsign-conversion -Werror` refuses —
    /// `combined-everything`'s suppressed union arm stopped compiling.
    /// The low octets the overlay writes are the same either way; only
    /// the parameter's spelling changes. Every other column's runtime
    /// takes the pattern signed (`long` in Java and C#) and keeps the
    /// default.
    fn bit_pattern(source: BitPatternSource, value: Self::Expr) -> Self::Expr {
        let value = match (&source, value) {
            (BitPatternSource::Int { .. }, c::Expr::Cast { value, .. }) => c::Expr::Cast {
                type_: Box::new(c::Type::Primitive(c::Primitive::UInt64)),
                value,
            },
            (_, other) => other,
        };
        match source {
            BitPatternSource::Int { octets } => Self::runtime_static(
                "bit_pattern_int",
                vec![value, Self::int(i128::from(octets))],
            ),
            BitPatternSource::Bool => Self::runtime_static("bit_pattern_bool", vec![value]),
            BitPatternSource::Float32 => Self::runtime_static("bit_pattern_f32", vec![value]),
            BitPatternSource::Float64 => Self::runtime_static("bit_pattern_f64", vec![value]),
        }
    }

    /// C has free functions, so this is [`CodecTarget::call`] with the
    /// runtime's prefix and no receiver.
    fn runtime_static(method: &str, args: Vec<Self::Expr>) -> Self::Expr {
        c::Expr::Call {
            callee: Box::new(c::Expr::Name(Ident::new(&format!(
                "{}{method}",
                Self::PREFIX
            )))),
            args,
        }
    }

    // -- expressions --------------------------------------------------

    fn name(text: &str) -> Self::Expr {
        c::Expr::Name(Ident::new(text))
    }

    fn int(value: i128) -> Self::Expr {
        c::Expr::Literal(Literal::int(value))
    }

    fn literal(value: Literal) -> Self::Expr {
        c::Expr::Literal(value)
    }

    fn call(callee: &str, args: Vec<Self::Expr>) -> Self::Expr {
        c::Expr::Call {
            callee: Box::new(c::Expr::Name(Ident::new(callee))),
            args,
        }
    }

    fn runtime(receiver: &str, method: &str, args: Vec<Self::Expr>) -> Self::Expr {
        let mut all = vec![c::Expr::Name(Ident::new(receiver))];
        all.extend(args);
        c::Expr::Call {
            callee: Box::new(c::Expr::Name(Ident::new(&format!(
                "{}{method}",
                Self::PREFIX
            )))),
            args: all,
        }
    }

    /// `runtime/c` carries the three HDDS axes: `zdw_begin_packed_element`
    /// and `zdw_finish_packed_element`, the DHEADER pair for a sequence
    /// element's own length, and `zdw_write_wstring_utf8_octets` /
    /// `zdw_read_wstring_utf8_octets`.
    fn carries_hdds_axes() -> bool {
        true
    }

    fn member(base: &str, field: &str) -> Self::Expr {
        c::Expr::Arrow {
            value: Box::new(c::Expr::Name(Ident::new(base))),
            name: Ident::new(field),
        }
    }

    fn whole(base: &str) -> Self::Expr {
        c::Expr::Deref(Box::new(c::Expr::Name(Ident::new(base))))
    }

    /// The suffix says which type the constant is; the cast at the use
    /// site says which type it is compared against.
    ///
    /// −2^63 has no such constant to suffix:
    /// `INT64_C(-9223372036854775808)` pastes the suffix onto digits C
    /// has no signed type for, and the minus then applies to an
    /// unsigned constant.
    /// [`zerodds_lang_render::c::integer_text`] carries the clause and
    /// the measurement, and answers with the subtraction `<stdint.h>`
    /// spells `INT64_MIN` as. A parenthesised expression and no longer
    /// a literal, which every site this reaches admits: the walk puts
    /// it inside a cast in an `if` condition and never where C requires
    /// a token — there is no `case` label in the generated C, the union
    /// reader is a chain of comparisons.
    fn wide_int(value: i128) -> Self::Expr {
        let text = if value > i128::from(i64::MAX) {
            format!("UINT64_C({value})")
        } else if value == i128::from(i64::MIN) {
            zerodds_lang_render::c::integer_text(value)
        } else if value > i128::from(i32::MAX) || value < i128::from(i32::MIN) {
            format!("INT64_C({value})")
        } else {
            format!("{value}")
        };
        c::Expr::Literal(Literal::Int(text))
    }

    fn field_of(value: Self::Expr, field: &str) -> Self::Expr {
        c::Expr::Member {
            value: Box::new(value),
            name: Ident::new(field),
        }
    }

    fn index(value: Self::Expr, at: Self::Expr) -> Self::Expr {
        c::Expr::Index {
            value: Box::new(value),
            index: Box::new(at),
        }
    }

    /// A C sequence keeps its length in the struct's `_length`; an
    /// array's bound is not kept anywhere, because the type carries it.
    fn count_of(base: &str, fixed: Option<u64>) -> Self::Expr {
        match fixed {
            Some(bound) => c::Expr::Literal(Literal::int(i128::from(bound))),
            None => Self::member(base, "_length"),
        }
    }

    fn binary(op: &'static str, lhs: Self::Expr, rhs: Self::Expr) -> Self::Expr {
        c::Expr::Binary {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        }
    }

    fn unary(op: &'static str, value: Self::Expr) -> Self::Expr {
        c::Expr::Unary {
            op,
            value: Box::new(value),
        }
    }

    fn paren(value: Self::Expr) -> Self::Expr {
        c::Expr::Paren(Box::new(value))
    }

    fn cast(type_: Self::Type, value: Self::Expr) -> Self::Expr {
        c::Expr::Cast {
            type_: Box::new(type_),
            value: Box::new(value),
        }
    }

    /// `&x` for a value and the array itself for an array — except
    /// where the value is already a dereference.
    ///
    /// `&*p` and `p` are the same pointer (C11 §6.5.3.2p3 says the two
    /// cancel and that neither is evaluated), and a member held behind
    /// an indirection reaches its value through
    /// [`C::behind_indirection`], so every call site that hands one to a
    /// codec would spell it `&*v->next`. The pair is dropped here rather
    /// than at the seam because the seam answers with the *value* — the
    /// encoder's `zdw_write_*` and the `sizeof` beside it both need it —
    /// and only the hand-off wants the address back.
    fn handoff(value: Self::Expr, form: Handoff) -> Self::Expr {
        match (form, value) {
            (Handoff::Array, value) => value,
            (Handoff::Value, c::Expr::Deref(inner)) => *inner,
            (Handoff::Value, value) => c::Expr::AddressOf(Box::new(value)),
        }
    }

    // -- statements ---------------------------------------------------

    fn declare(type_: Self::Type, name: &str, value: Option<Self::Expr>) -> Self::Stmt {
        c::Statement::Declare {
            type_,
            name: Ident::new(name),
            value,
        }
    }

    fn perform(value: Self::Expr) -> Self::Stmt {
        c::Statement::Expr(value)
    }

    fn give_back(value: Option<Self::Expr>) -> Self::Stmt {
        c::Statement::Return(value)
    }

    fn when(
        condition: Self::Expr,
        then: Self::Block,
        otherwise: Option<Self::Block>,
    ) -> Self::Stmt {
        c::Statement::If {
            condition,
            then,
            otherwise,
        }
    }

    /// `for (uint32_t i = 0; i < bound; i = i + 1)`, the counter
    /// declared in the initialiser.
    ///
    /// Two things here are older than this backend and are kept because
    /// changing either moves the generated text without moving a byte:
    /// the counter is `uint32_t` rather than `size_t`, which is the type
    /// a sequence length has on the wire and keeps `-Wsign-conversion`
    /// quiet where it is compared against one; and the step is written
    /// out rather than `i++`, which the tree has no node for.
    fn count_up(counter: &str, bound: Self::Expr, body: Self::Block) -> Self::Stmt {
        c::Statement::For {
            init: Box::new(c::Statement::Declare {
                type_: c::Type::Primitive(c::Primitive::UInt32),
                name: Ident::new(counter),
                value: Some(c::Expr::Literal(Literal::int(0))),
            }),
            condition: c::Expr::Binary {
                op: "<",
                lhs: Box::new(c::Expr::Name(Ident::new(counter))),
                rhs: Box::new(bound),
            },
            step: c::Expr::Binary {
                op: "=",
                lhs: Box::new(c::Expr::Name(Ident::new(counter))),
                rhs: Box::new(c::Expr::Binary {
                    op: "+",
                    lhs: Box::new(c::Expr::Name(Ident::new(counter))),
                    rhs: Box::new(c::Expr::Literal(Literal::int(1))),
                }),
            },
            body,
        }
    }

    fn until(condition: Self::Expr, body: Self::Block) -> Self::Stmt {
        c::Statement::While { condition, body }
    }

    fn leave() -> Self::Stmt {
        c::Statement::Break
    }

    fn note(doc: Doc) -> Self::Stmt {
        c::Statement::Comment(doc)
    }

    // -- assembly -----------------------------------------------------

    /// The five statements C needs, with the fill inside the guard.
    ///
    /// `zdw_reader_alloc` takes from a buffer the caller set, so a
    /// failure is an exhausted arena rather than an exhausted machine —
    /// which is why it returns nothing rather than aborting, and why
    /// the generated reader has to check.
    fn make_room(
        base: &str,
        element: Self::Type,
        count: Self::Expr,
        // C's slot is storage the moment the buffer is there, so
        // whether the fill assigns it or writes into it makes no
        // difference to what has to be allocated.
        _assigned_whole: bool,
        fill: Vec<Self::Stmt>,
    ) -> Vec<Self::Stmt> {
        let field = |name: &str| Self::member(base, name);
        let assign = |place: c::Expr, value: c::Expr| {
            c::Statement::Expr(c::Expr::Binary {
                op: "=",
                lhs: Box::new(place),
                rhs: Box::new(value),
            })
        };
        let mut guarded = vec![
            assign(
                field("_buffer"),
                c::Expr::Cast {
                    type_: Box::new(c::Type::Pointer(Box::new(element.clone()))),
                    value: Box::new(Self::runtime(
                        "r",
                        "reader_alloc",
                        vec![c::Expr::Binary {
                            op: "*",
                            lhs: Box::new(c::Expr::Cast {
                                type_: Box::new(c::Type::Primitive(c::Primitive::Size)),
                                value: Box::new(count.clone()),
                            }),
                            rhs: Box::new(c::Expr::SizeOf(Box::new(element))),
                        }],
                    )),
                },
            ),
            c::Statement::If {
                condition: c::Expr::Binary {
                    op: "==",
                    lhs: Box::new(field("_buffer")),
                    rhs: Box::new(Self::absent()),
                },
                then: c::Block {
                    statements: vec![c::Statement::Return(None)],
                },
                otherwise: None,
            },
        ];
        guarded.extend(fill);
        // The capacity is the count, and the arena the buffer is in is not the sequence's to free.
        vec![
            assign(field("_maximum"), count.clone()),
            assign(field("_release"), c::Expr::Literal(Literal::Bool(false))),
            assign(field("_length"), count.clone()),
            assign(field("_buffer"), Self::absent()),
            c::Statement::If {
                condition: c::Expr::Binary {
                    op: ">",
                    lhs: Box::new(count),
                    rhs: Box::new(c::Expr::Literal(Literal::Int("0u".to_string()))),
                },
                then: c::Block {
                    statements: guarded,
                },
                otherwise: None,
            },
        ]
    }

    /// `v->x != NULL`. XTypes 1.3 §7.5.1.2.4.1: "A NULL pointer shall
    /// indicate an omitted value."
    fn present(base: &str, field: &str) -> Self::Expr {
        c::Expr::Binary {
            op: "!=",
            lhs: Box::new(Self::member(base, field)),
            rhs: Box::new(Self::absent()),
        }
    }

    /// XTypes 1.3 §7.5.1.4.1.3: C binds a derived structure with the base as its first member
    /// `parent`, so a member of the base is `parent.m` and one of its base `parent.parent.m`.
    fn inherited_path(depth: usize) -> Option<String> {
        Some(format!("{}.", crate::adapter::c::PARENT).repeat(depth))
    }

    /// Nothing on the way in — [`C::make_indirection`] allocates, and a
    /// `string` is assigned by its own read — and `v->x = NULL;` on the
    /// way out.
    fn set_present(base: &str, field: &str, present: bool) -> Vec<Self::Stmt> {
        if present {
            return Vec::new();
        }
        vec![c::Statement::Expr(c::Expr::Binary {
            op: "=",
            lhs: Box::new(Self::member(base, field)),
            rhs: Box::new(Self::absent()),
        })]
    }

    /// An `@optional` member is a pointer, `T *`, and a `string` is the
    /// `const char *` it already is.
    fn optionals_are_pointers() -> bool {
        true
    }

    /// `if (!(v->x != NULL)) { zdw_writer_refuse(w, ZDW_UNSET_EXTERNAL); return; }`
    fn refuse_unset_external(present: Self::Expr) -> Vec<Self::Stmt> {
        vec![c::Statement::If {
            condition: c::Expr::Unary {
                op: "!",
                value: Box::new(c::Expr::Paren(Box::new(present))),
            },
            then: c::Block {
                statements: vec![
                    c::Statement::Expr(Self::runtime(
                        "w",
                        "writer_refuse",
                        vec![c::Expr::Name(Ident::new("ZDW_UNSET_EXTERNAL"))],
                    )),
                    c::Statement::Return(None),
                ],
            },
            otherwise: None,
        }]
    }

    fn case_member(base: &str, field: &str) -> Self::Expr {
        c::Expr::Member {
            value: Box::new(Self::member(base, "_u")),
            name: Ident::new(field),
        }
    }

    fn entry_half(entry: Self::Expr, key: bool) -> Self::Expr {
        c::Expr::Member {
            value: Box::new(entry),
            name: Ident::new(if key { "key" } else { "value" }),
        }
    }

    fn optional_value(_ast: &Ast, _id: Id<TypeNode>, base: &str, field: &str) -> Self::Expr {
        Self::member(base, field)
    }

    /// `v->_buffer[i]`, and nothing around it. Every C object is
    /// addressable, so there is no proxy to work around.
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

    /// `v->_length = count;`. The elements past it stay in the buffer,
    /// unreferenced, and the length is what says the sequence stops.
    fn keep_first(base: &str, count: Self::Expr) -> Vec<Self::Stmt> {
        vec![c::Statement::Expr(c::Expr::Binary {
            op: "=",
            lhs: Box::new(Self::count_of(base, None)),
            rhs: Box::new(count),
        })]
    }

    /// The member, and nothing to do first: for a `string` the member is
    /// the `const char *` the read assigns. Every other optional member is
    /// a pointer (XTypes 1.3 §7.5.1.2.4.1) and [`C::make_indirection`] is
    /// where it is made to point somewhere before it is written through.
    fn optional_destination(base: &str, field: &str) -> (Vec<Self::Stmt>, Self::Expr) {
        (Vec::new(), Self::member(base, field))
    }

    /// `*v->next` — the member dereferenced.
    ///
    /// `crates/lang-rules/src/adapter/c.rs` declares a member that
    /// closes a cycle as `T *`, because a C structure holds its members
    /// as storage and `T next;` inside `T` is a structure that contains
    /// itself: `field 'next' has incomplete type`. The pointer is what
    /// gives the type a size, and `@external` is the annotation the case
    /// carries for exactly that reason — XTypes 1.3 Table 21, "stored
    /// externally to the containing type". No octet moves: the codec
    /// takes the value either way, and this is where the value is.
    fn externals_are_pointers() -> bool {
        true
    }

    fn behind_indirection(_ast: &Ast, _id: Id<TypeNode>, place: Self::Expr) -> Self::Expr {
        c::Expr::Deref(Box::new(place))
    }

    /// The same place, with the pointer made to point somewhere first.
    ///
    /// ```c
    /// v->next = (m_Node *)zdw_reader_alloc(r, sizeof(m_Node));
    /// if (v->next == NULL) { return; }
    /// ```
    ///
    /// The allocation is [`CodecTarget::make_room`]'s, out of the buffer
    /// the caller handed the reader: a failure is an exhausted arena and
    /// not an exhausted machine, so it is a value that is not there and
    /// the generated reader checks. Nothing frees it, for the reason
    /// nothing frees a sequence's buffer — the arena is the caller's and
    /// outlives the read.
    fn make_indirection(
        ast: &Ast,
        id: Id<TypeNode>,
        place: Self::Expr,
    ) -> (Vec<Self::Stmt>, Self::Expr) {
        let held = Self::value_type(ast, id);
        let made = c::Statement::Expr(c::Expr::Binary {
            op: "=",
            lhs: Box::new(place.clone()),
            rhs: Box::new(c::Expr::Cast {
                type_: Box::new(c::Type::Pointer(Box::new(held.clone()))),
                value: Box::new(Self::runtime(
                    "r",
                    "reader_alloc",
                    vec![c::Expr::SizeOf(Box::new(held))],
                )),
            }),
        });
        let guard = c::Statement::If {
            condition: c::Expr::Binary {
                op: "==",
                lhs: Box::new(place.clone()),
                rhs: Box::new(Self::absent()),
            },
            then: c::Block {
                statements: vec![c::Statement::Return(None)],
            },
            otherwise: None,
        };
        (vec![made, guard], c::Expr::Deref(Box::new(place)))
    }

    fn elements(base: &str) -> Self::Expr {
        Self::member(base, "_buffer")
    }

    fn absent() -> Self::Expr {
        c::Expr::Name(Ident::new("NULL"))
    }

    /// `v != NULL`. The reader allocates out of a buffer the caller
    /// set, and a buffer that runs out is a value that is not there.
    fn materialised(value: Self::Expr) -> Option<Self::Expr> {
        Some(c::Expr::Binary {
            op: "!=",
            lhs: Box::new(value),
            rhs: Box::new(Self::absent()),
        })
    }

    fn block(statements: Vec<Self::Stmt>) -> Self::Block {
        c::Block { statements }
    }

    fn into_statements(block: Self::Block) -> Vec<Self::Stmt> {
        block.statements
    }

    /// `static void zd_enc_T(zdw_writer *w, const T *v)`.
    ///
    /// `static` because a probe's translation unit is one file and an
    /// external codec nothing else declares is a warning under the flags
    /// the probe compiles with.
    fn routine(spec: Routine<Self>) -> Option<Self::Decl> {
        let stream = c::Param {
            type_: c::Type::Pointer(Box::new(c::Type::Named(Ident::new(if spec.writing {
                "zdw_writer"
            } else {
                "zdw_reader"
            })))),
            name: Some(Ident::new(if spec.writing { "w" } else { "r" })),
        };
        // The written direction may not touch the sample and the read
        // direction must; `const` is what says so, and dropping it would
        // let a walk assign through a parameter it was handed to read.
        let constant = |type_: c::Type| {
            if spec.writing {
                c::Type::Const(Box::new(type_))
            } else {
                type_
            }
        };
        // An array parameter is adjusted to a pointer to its first
        // element on its own, so it is spelled as the array and indexed
        // directly; `(*v)[i][j]` would otherwise need parentheses the
        // tree has no node for.
        let value = match spec.held {
            Handoff::Array => constant(spec.sample),
            Handoff::Value => c::Type::Pointer(Box::new(constant(spec.sample))),
        };
        let mut params = vec![
            stream,
            c::Param {
                type_: value,
                name: Some(Ident::new("v")),
            },
        ];
        params.extend(spec.extra.into_iter().map(|(type_, name)| c::Param {
            type_,
            name: Some(Ident::new(&name)),
        }));
        Some(c::Decl::Function(c::Function {
            doc: spec.doc,
            is_static: true,
            is_inline: false,
            is_maybe_unused: spec.tolerate_unused,
            returns: c::Type::Void,
            name: Ident::new(&spec.name),
            params,
            body: spec.body,
        }))
    }

    // -- the materialiser's boundary ----------------------------------

    fn value_type(ast: &Ast, id: Id<TypeNode>) -> Self::Type {
        c_value_type(ast, id)
    }

    /// [`c_codec_tag`] and not [`crate::adapter::c::c_type_name`]: this names the routine a
    /// type's codec is emitted under, and the two part on a string. A
    /// `string<4>` and a `string` are both a `zd_string` — one C type,
    /// no declaration of its own — so the tag is the only place their
    /// bound can keep their routines apart. Everywhere the answer is
    /// used as a *spelling* it is an aggregate's, where the two
    /// functions agree.
    fn type_name(ast: &Ast, id: Id<TypeNode>) -> String {
        c_codec_tag(ast, id)
    }

    fn escape(name: &str) -> String {
        c::escape_keyword(name)
    }

    fn map_entry_name(ast: &Ast, id: Id<TypeNode>) -> String {
        c_map_entry_name(ast, id)
    }

    /// The enclosing modules flattened onto the bare enumerator, which
    /// is where a C enumeration constant lives — C11 §6.7.2.2 puts it
    /// in the scope enclosing the enumeration and not in the
    /// enumeration.
    fn enumerator_constant(ast: &Ast, id: Id<TypeNode>, name: &str) -> Self::Expr {
        Self::name(&enumerator_constant_name(ast, id, name))
    }

    fn module_prefix(ast: &Ast, id: Id<TypeNode>) -> String {
        module_prefix(ast, id)
    }

    /// Never refuses: `default_c` emits a defaulter for every type the
    /// C materialiser can declare, and a type it cannot declare has
    /// already been refused before a decoder is asked for.
    fn default_into(
        ast: &Ast,
        id: Id<TypeNode>,
        place: Self::Expr,
    ) -> Result<Self::Stmt, EncodeError> {
        Ok(crate::default_c::default_into(ast, id, place))
    }

    fn member_default(
        ast: &Ast,
        node: &MemberNode,
        type_: Id<TypeNode>,
        place: Self::Expr,
    ) -> Result<Vec<Self::Stmt>, EncodeError> {
        crate::default_c::member_default(ast, node, type_, place)
    }
}

/// The name a C codec function takes, given the runtime's prefix.
///
/// Exposed because a caller that emits a prototype and a body has to
/// agree with the walk on the spelling, and two `format!`s in two files
/// would be two chances to part over an underscore.
#[must_use]
pub fn runtime_function(method: &str) -> String {
    format!("{}{method}", C::PREFIX)
}

/// The same, as text.
///
/// `default_c` writes the constant into an initialiser rather than into
/// an expression tree, and a second `format!` in that file would be a
/// second place the C spelling is decided.
pub(crate) fn enumerator_constant_name(ast: &Ast, id: Id<TypeNode>, name: &str) -> String {
    format!("{}{}", module_prefix(ast, id), c::escape_keyword(name))
}
