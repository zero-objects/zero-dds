// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Julia backend of the shared codec walk.
//!
//! DDS-XTypes 1.3 §7.4 from the clause, the same as `runtime/c`,
//! `runtime/python`, `runtime/typescript` and the rest do, and none of
//! them calls another. Where two columns disagree on an octet, one has
//! misread the specification — which is the only reason to have another.
//!
//! # The three decisions this column turns on
//!
//! **The declared types are immutable and the decoder still assigns
//! fields.** `adapter/julia.rs` declares every aggregate as an immutable
//! `struct` — a decoded sample is a value, not a handle — and the shared
//! walk writes `v.field = …` at every member. The two meet in the
//! runtime's `ZdScratch`: a decoder's `v` starts as the Table 9 scratch
//! its defaulter answers (`crate::default_julia`), every store lands in
//! the scratch's dictionary, and each `return` goes through
//! `zd_freeze(zd_rt, v)`, which builds the immutable value by the
//! declared field order. `zd_rt` is a local the reading body binds to
//! the sample's own type, because [`CodecTarget::give_back`] has no type
//! parameter to name it with.
//!
//! **A collection under construction is a `Vector{Any}`.** An IDL array
//! is an `NTuple` — the one shape Julia's own type system fixes the
//! length of — and an `NTuple` cannot be written into element by
//! element. The reader therefore fills `Any`-vectors and `zd_freeze`
//! converts, dimension by dimension. A `map`'s entry under construction
//! is a `ZdScratch` with `key` and `value` slots for the same reason,
//! so [`CodecTarget::entry_half`] is one spelling — `.key` / `.value` —
//! in both directions: a field access reaches a real `ZdMapEntry_…` on
//! the way out and the scratch's dictionary on the way in.
//!
//! **A subscript adds one.** Julia counts from one and the walk counts
//! from zero; the counter stays the walk's and the `+ 1` is written at
//! the subscript, in [`CodecTarget::index`], so an index that is also
//! arithmetic — a rank, a position — keeps the walk's meaning.
//!
//! # Two width traps the spellings guard against
//!
//! - `for i in 0:(n - 1)` with an unsigned `n` of zero wraps to
//!   `0:0xffffffff` and runs four billion iterations, so every counted
//!   loop bound goes through `Int64(…)` first.
//! - a bare `1 << 40` is a platform `Int`, and a small hex literal is a
//!   small unsigned type whose `~` loses the high bits, so every 64-bit
//!   pattern is written `UInt64(…)`.

use super::{CodecTarget, Constant, Handle, Handoff, Routine, Scalar};
use crate::adapter::julia::{
    julia_map_entry_name, julia_module_prefix, julia_type_name, julia_value_type,
};
use crate::encode::EncodeError;
use zerodds_ast::{Ast, CharWidth, MemberNode, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::{Doc, Ident, Literal};
use zerodds_lang_cst::julia::{self as jl, escape_field, escape_name};

/// The Julia backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Julia;

/// How deep a chain of typedefs and collections is followed.
const MAX_TYPE_DEPTH: usize = 64;

/// The type name [`CodecTarget::named`] is handed for C's signed `char`.
///
/// `emit/encode.rs`'s `map_key_less` writes `T::named("signed char")`
/// where a `map`'s key is a `char` and the entry order is
/// `MapKeyOrdering::SignedInteger`. That is a C spelling in the walk and
/// not a type Julia has; this column recognises the string and answers
/// with the two's-complement arithmetic. See [`signed_char`].
const SIGNED_CHAR: &str = "signed char";

/// What [`CodecTarget::map_entry_name`] answers with.
///
/// Not a type this column declares. It is a marker the one seam that
/// receives it — [`Julia::storage`], through the default
/// `map_entry_type` — matches on, because a `map` entry under
/// construction is a runtime `ZdScratch` and not the materialiser's
/// `ZdMapEntry_…` struct, which is immutable and cannot take one half
/// at a time.
const MAP_ENTRY: &str = "_ZdMapEntryScratch";

impl Julia {
    /// A body's statements, unchanged: Julia accepts an empty block.
    fn suite(body: Vec<jl::Statement>) -> Vec<jl::Statement> {
        body
    }

    /// A reader's body, with the scratch it fills and the freeze that
    /// answers.
    ///
    /// `zd_rt = <SampleType>` first, so that every `return` the walk
    /// builds — [`Julia::give_back`] has no type to hand — can say
    /// `return zd_freeze(zd_rt, v)`; then `v = zd_def_…()`, Table 9's
    /// scratch through the generated defaulter; then the walk's own
    /// statements; then the freeze.
    fn reading_body(name: &str, sample: jl::Type, body: Vec<jl::Statement>) -> Vec<jl::Statement> {
        let defaulter = crate::default_julia::defaulter_from_decoder(name);
        let mut statements = vec![
            jl::Statement::Assign {
                place: jl::Expr::name("zd_rt"),
                value: jl::Expr::Type(Box::new(sample)),
            },
            jl::Statement::Assign {
                place: jl::Expr::name("v"),
                value: jl::Expr::free_call(&defaulter, Vec::new()),
            },
        ];
        statements.extend(body);
        statements.push(Self::give_back(None));
        statements
    }

    /// A policy value, as the string the runtime accepts.
    fn text(value: &str) -> jl::Expr {
        jl::Expr::Literal(Literal::Str(value.to_string()))
    }
}

impl CodecTarget for Julia {
    type Expr = jl::Expr;
    type Stmt = jl::Statement;
    type Block = Vec<jl::Statement>;
    type Type = jl::Type;
    type Decl = jl::Decl;

    // -- types --------------------------------------------------------

    /// The fixed-width holders `adapter/julia.rs` declares, and `Int64`
    /// for the walk's own counters and positions.
    ///
    /// Never `Int`: that is `Int64` on this host and `Int32` on a
    /// 32-bit one, for a wire format that fixes every width. A `char`
    /// is a `UInt8` code unit and a `long double` a `String` holding
    /// the decimal — both the materialiser's answers, restated here so
    /// a codec's local and a struct's field are one type.
    fn scalar(kind: Scalar) -> Self::Type {
        jl::Type::Primitive(match kind {
            Scalar::Bool => jl::Primitive::Bool,
            Scalar::Char | Scalar::UInt8 => jl::Primitive::UInt8,
            Scalar::Int8 => jl::Primitive::Int8,
            Scalar::Int16 => jl::Primitive::Int16,
            Scalar::UInt16 => jl::Primitive::UInt16,
            Scalar::Int32 => jl::Primitive::Int32,
            Scalar::UInt32 => jl::Primitive::UInt32,
            Scalar::UInt64 => jl::Primitive::UInt64,
            // The walk's counters, byte positions and the enumeration
            // holder. Signed on purpose: `0:(Int64(n) - 1)` cannot wrap
            // where an unsigned zero would.
            Scalar::Int64 | Scalar::Index | Scalar::Size => jl::Primitive::Int64,
            Scalar::Float => jl::Primitive::Float32,
            Scalar::Double => jl::Primitive::Float64,
            Scalar::LongDouble => jl::Primitive::String,
        })
    }

    fn named(name: &str) -> Self::Type {
        jl::Type::named(name)
    }

    /// `Nothing`, whose one value is what a Julia function with no
    /// answer returns.
    fn nothing() -> Self::Type {
        jl::Type::Primitive(jl::Primitive::Nothing)
    }

    /// The runtime's handles, by their exported names.
    fn handle(kind: Handle) -> Self::Type {
        jl::Type::named(match kind {
            Handle::DHeader => "ZdDHeader",
            Handle::EmHeader => "ZdEmHeader",
            Handle::Pid => "ZdPid",
            Handle::TryConstruct => "ZdTryConstruct",
            Handle::ReaderMark => "ZdReaderMark",
            Handle::PidRead => "ZdPidRead",
            Handle::EmHeaderRead => "ZdEmHeaderRead",
        })
    }

    /// The code unit, unchanged.
    ///
    /// `adapter/julia.rs` holds a `char` in a `UInt8` and a `wchar` in
    /// a `UInt16`, and `runtime/julia`'s `write_char` and `write_wchar`
    /// take the integer, so there is nothing to convert on either side.
    fn code_point(value: Self::Expr, _narrow: bool) -> Self::Expr {
        value
    }

    /// The runtime's policy values, in the spelling `runtime/julia`
    /// accepts.
    ///
    /// Strings and integers for the wire-form axes — the same spellings
    /// `runtime/python` takes, because both are
    /// `WstringLengthPolicy::as_str` and its siblings in
    /// `crates/ast-layout/src/key.rs` — and the runtime's `@enum`
    /// instances for the roles, behaviours and statuses. One arm per
    /// constant and no catch-all: a policy this column has not
    /// implemented has to be a compile error here rather than a value
    /// that looks plausible.
    fn constant(which: Constant) -> Self::Expr {
        match which {
            Constant::WcharWidth2 => jl::Expr::int(2),
            Constant::WcharWidth4 => jl::Expr::int(4),
            Constant::WcharFixed => Self::text("fixed"),
            Constant::WcharGiopLengthPrefixed => Self::text("giop-length-prefixed"),
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
            Constant::ParameterMember => jl::Expr::name("ZD_PARAMETER_MEMBER"),
            Constant::ParameterOptional => jl::Expr::name("ZD_PARAMETER_OPTIONAL"),
            Constant::TryConstructDiscard => jl::Expr::name("ZD_TRY_CONSTRUCT_DISCARD"),
            Constant::TryConstructTrim => jl::Expr::name("ZD_TRY_CONSTRUCT_TRIM"),
            Constant::TryConstructUseDefault => jl::Expr::name("ZD_TRY_CONSTRUCT_USE_DEFAULT"),
            Constant::StatusOk => jl::Expr::name("ZD_STATUS_OK"),
            Constant::StatusDiscarded => jl::Expr::name("ZD_STATUS_DISCARDED"),
            Constant::StatusImpossibleLength => jl::Expr::name("ZD_STATUS_IMPOSSIBLE_LENGTH"),
            Constant::StatusOutOfRange => jl::Expr::name("ZD_STATUS_OUT_OF_RANGE"),
            Constant::StatusTruncated => jl::Expr::name("ZD_STATUS_TRUNCATED"),
        }
    }

    /// The smallest unsigned integer that holds `bits`.
    fn unsigned_holder(bits: u16) -> Self::Type {
        jl::Type::Primitive(match bits {
            0..=8 => jl::Primitive::UInt8,
            9..=16 => jl::Primitive::UInt16,
            17..=32 => jl::Primitive::UInt32,
            _ => jl::Primitive::UInt64,
        })
    }

    fn value_type(ast: &Ast, id: Id<TypeNode>) -> Self::Type {
        julia_value_type(ast, id).unwrap_or(jl::Type::Primitive(jl::Primitive::Any))
    }

    fn type_name(ast: &Ast, id: Id<TypeNode>) -> String {
        codec_tag(ast, id, 0)
    }

    /// The scratch marker and never the materialiser's entry struct:
    /// the struct is immutable and a reader fills an entry one half at
    /// a time. See `MAP_ENTRY`.
    fn map_entry_name(_ast: &Ast, _id: Id<TypeNode>) -> String {
        MAP_ENTRY.to_string()
    }

    /// The flattened module path, which in this column *is* the scope:
    /// `adapter/julia.rs` writes no `module` and every declaration
    /// carries its path in its name.
    fn module_prefix(ast: &Ast, id: Id<TypeNode>) -> String {
        julia_module_prefix(ast, id)
    }

    /// The field spelling, which escapes keywords only. A field lives
    /// in the struct's own namespace and shadows nothing —
    /// `zerodds_lang_cst::julia::escape_field` carries the measurement.
    fn escape(name: &str) -> String {
        escape_field(name)
    }

    /// The base is the first member `parent` (XTypes 1.3 §7.5.1.4.1.3).
    fn inherited_path(depth: usize) -> Option<String> {
        Some(format!("{}.", escape_field("parent")).repeat(depth))
    }

    // -- expressions ---------------------------------------------------

    fn name(text: &str) -> Self::Expr {
        jl::Expr::name(text)
    }

    fn int(value: i128) -> Self::Expr {
        jl::Expr::int(value)
    }

    /// The number, unsuffixed.
    ///
    /// Julia parses a decimal past `typemax(Int64)` as the next wider
    /// signed type, and a comparison or a typed-local assignment
    /// converts exactly, so `16045690984833335998` needs no suffix and
    /// no widening call.
    fn wide_int(value: i128) -> Self::Expr {
        jl::Expr::int(value)
    }

    /// `UInt64(<n>)` — converted, never bare.
    ///
    /// A bare hex literal takes the smallest unsigned type it fits, so
    /// `~0x07` is a `UInt8` whose complement has lost fifty-six bits by
    /// the time it meets a `UInt64`; and a bare decimal is a platform
    /// `Int`. The conversion is exact for every `u64`.
    fn wide_unsigned(value: u64) -> Self::Expr {
        jl::Expr::convert(
            jl::Type::Primitive(jl::Primitive::UInt64),
            jl::Expr::int(i128::from(value)),
        )
    }

    /// As [`Self::wide_int`]: the value survives its own width without
    /// a suffix.
    fn wide_signed(value: i128) -> Self::Expr {
        jl::Expr::int(value)
    }

    /// The number, with no suffix: `2u` is not a Julia token.
    fn unsigned_int(value: i128) -> Self::Expr {
        jl::Expr::int(value)
    }

    /// A literal, with the two raw tokens the walk escapes through
    /// [`Literal::Int`] rewritten.
    ///
    /// `emit/decode.rs`'s `literal::<T>("false")` carries a keyword in
    /// an integer literal because that variant is the tree's raw-token
    /// escape. Julia spells the booleans the same way C does, so the
    /// rewrite only moves them into [`Literal::Bool`], where the
    /// renderer owns the spelling.
    fn literal(value: Literal) -> Self::Expr {
        let Literal::Int(text) = &value else {
            return jl::Expr::Literal(value);
        };
        match text.as_str() {
            "true" => jl::Expr::Literal(Literal::Bool(true)),
            "false" => jl::Expr::Literal(Literal::Bool(false)),
            _ => jl::Expr::Literal(value),
        }
    }

    /// A call to a module-level function.
    ///
    /// `strcmp` is the one name the walk hands here that is not a
    /// function this column generates, and it is C's. `Base.cmp` on two
    /// `String`s compares byte-wise — UTF-8, which C11 §7.24.4's
    /// `unsigned char` order preserves — and answers the same sign.
    fn call(callee: &str, args: Vec<Self::Expr>) -> Self::Expr {
        if callee == "strcmp" {
            return jl::Expr::free_call("Base.cmp", args);
        }
        jl::Expr::free_call(callee, args)
    }

    /// `write_u32(w, x)` — the runtime is a set of free functions and
    /// the receiver is the first argument, which is Julia's own calling
    /// convention.
    fn runtime(receiver: &str, method: &str, args: Vec<Self::Expr>) -> Self::Expr {
        // The walk's bare `read_pid()` names the operation that opens
        // no value block, because the column it was written for opens
        // one itself. This walk has no such pair — it reads a header
        // and later calls `finish_pid_read` — so the block has to be
        // opened by the read, which is what `read_pid_as` does.
        // §7.4.1.2 makes the difference a wire question: a member's
        // internal alignment counts from the first octet after its
        // header.
        if method == "read_pid" && args.is_empty() {
            return Self::runtime(
                receiver,
                "read_pid_as",
                vec![Self::constant(Constant::ParameterMember)],
            );
        }
        let mut all = vec![jl::Expr::name(receiver)];
        all.extend(args);
        jl::Expr::free_call(method, all)
    }

    /// A free function of the runtime — `no_dheader`, `utf8_octets`,
    /// `pid_is_user_member`, the `bit_pattern_*` family — which is
    /// exactly what `runtime/julia` exports them as.
    fn runtime_static(method: &str, args: Vec<Self::Expr>) -> Self::Expr {
        jl::Expr::free_call(method, args)
    }

    /// `runtime/julia` carries the three HDDS axes:
    /// `begin_packed_element` / `finish_packed_element` on the writer
    /// and `reader_begin_packed_element` / `reader_finish_packed_element`
    /// on the reader, the DHEADER pair for a sequence element's own
    /// length, and `write_wstring_utf8_octets` /
    /// `write_wstring_utf8_octets_bounded` / `read_wstring_utf8_octets`.
    fn carries_hdds_axes() -> bool {
        true
    }

    fn member(base: &str, field: &str) -> Self::Expr {
        jl::Expr::field(jl::Expr::name(base), field)
    }

    fn whole(base: &str) -> Self::Expr {
        jl::Expr::name(base)
    }

    fn field_of(value: Self::Expr, field: &str) -> Self::Expr {
        jl::Expr::field(value, field)
    }

    /// `value[(at) + 1]`: the walk counts from zero and Julia from one,
    /// and the conversion lives at the subscript so the counter itself
    /// keeps the walk's meaning everywhere else it is read.
    fn index(value: Self::Expr, at: Self::Expr) -> Self::Expr {
        jl::Expr::index(
            value,
            jl::Expr::Binary {
                op: "+",
                lhs: Box::new(jl::Expr::Paren(Box::new(at))),
                rhs: Box::new(jl::Expr::int(1)),
            },
        )
    }

    /// `Base.length(v)`, or the bound where the materialiser fixed one.
    ///
    /// Qualified, because a generated file binds its declarations at
    /// the scope this codec runs in and an IDL type named `length`
    /// would take the bare name over.
    fn count_of(base: &str, fixed: Option<u64>) -> Self::Expr {
        match fixed {
            Some(bound) => jl::Expr::int(i128::from(bound)),
            None => jl::Expr::free_call("Base.length", vec![jl::Expr::name(base)]),
        }
    }

    fn elements(base: &str) -> Self::Expr {
        jl::Expr::name(base)
    }

    /// `v.field` — a union's selected case member. `adapter/julia.rs`
    /// declares one `Union{T, Nothing}` field per case on the struct.
    fn case_member(base: &str, field: &str) -> Self::Expr {
        Self::member(base, field)
    }

    /// `.key` and `.value` — one spelling for both directions.
    ///
    /// The encoder reaches a real `ZdMapEntry_…` field; the decoder
    /// reaches the scratch's dictionary through the same property
    /// syntax, which is what `ZdScratch` overloads `getproperty` for.
    fn entry_half(entry: Self::Expr, key: bool) -> Self::Expr {
        jl::Expr::field(entry, if key { "key" } else { "value" })
    }

    fn absent() -> Self::Expr {
        jl::Expr::Literal(Literal::Null)
    }

    /// Nothing: a Julia `String` owns its storage and a read that
    /// produced one produced it whole.
    fn materialised(_value: Self::Expr) -> Option<Self::Expr> {
        None
    }

    /// The member itself: `Union{T, Nothing}` is the value or
    /// `nothing`, with no wrapper to unwrap.
    fn optional_value(_ast: &Ast, _id: Id<TypeNode>, base: &str, field: &str) -> Self::Expr {
        Self::member(base, field)
    }

    /// `v.field !== nothing`.
    ///
    /// `!==` and not `!=`: `!=` calls `==`, which for `missing`-like
    /// values and overloaded types answers structurally, while identity
    /// against the one value of `Nothing` is the question being asked.
    fn present(base: &str, field: &str) -> Self::Expr {
        jl::Expr::Binary {
            op: "!==",
            lhs: Box::new(Self::member(base, field)),
            rhs: Box::new(Self::absent()),
        }
    }

    /// Nothing on the way in, `v.field = nothing` on the way out:
    /// writing the member is what makes it present.
    fn set_present(base: &str, field: &str, present: bool) -> Vec<Self::Stmt> {
        if present {
            return Vec::new();
        }
        vec![jl::Statement::Assign {
            place: Self::member(base, field),
            value: Self::absent(),
        }]
    }

    /// `v.field = nothing` for every case: `adapter/julia.rs` declares
    /// one `Union{T, Nothing}` field per case, and only the selected one
    /// holds a value. The same statement as an absent
    /// `@optional` member, [`Self::set_present`] with `false`.
    fn clear_union_cases(base: &str, fields: &[String]) -> Vec<Self::Stmt> {
        fields
            .iter()
            .flat_map(|field| Self::set_present(base, field, false))
            .collect()
    }

    /// The member, with nothing to do first: the scratch's slot exists
    /// the moment it is named.
    fn optional_destination(base: &str, field: &str) -> (Vec<Self::Stmt>, Self::Expr) {
        (Vec::new(), Self::member(base, field))
    }

    /// The operators, unchanged: Julia spells `&&`, `||`, `==`, `!=`,
    /// the shifts and the bitwise pair exactly as the walk writes them.
    /// `=` is left as it is and never reaches a rendered expression:
    /// [`Self::perform`] is the only thing that receives one and turns
    /// it into a statement.
    fn binary(op: &'static str, lhs: Self::Expr, rhs: Self::Expr) -> Self::Expr {
        jl::Expr::Binary {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        }
    }

    fn unary(op: &'static str, value: Self::Expr) -> Self::Expr {
        jl::Expr::Unary {
            op,
            value: Box::new(value),
        }
    }

    fn paren(value: Self::Expr) -> Self::Expr {
        jl::Expr::Paren(Box::new(value))
    }

    /// A conversion, written as the constructor call Julia spells one
    /// with — and three special cases that are not conversions.
    ///
    /// - `signed char` is the walk's C spelling of a map key order and
    ///   becomes the two's-complement arithmetic (`signed_char`);
    /// - `Int64` goes through the runtime's `zd_i64`, because
    ///   `Int64(x::UInt64)` throws above `typemax(Int64)` where the
    ///   walk means the bit pattern — the union overlay reads an
    ///   eight-octet unsigned arm through exactly this seam;
    /// - a `Bool` target with an integer literal is the label rewrite
    ///   at a boolean discriminator: `true`/`false`, not `Bool(1)`.
    fn cast(type_: Self::Type, value: Self::Expr) -> Self::Expr {
        if type_ == jl::Type::named(SIGNED_CHAR) {
            return signed_char(value);
        }
        if type_ == jl::Type::Primitive(jl::Primitive::Int64) {
            return jl::Expr::free_call("zd_i64", vec![value]);
        }
        if type_ == jl::Type::Primitive(jl::Primitive::Bool) {
            if let jl::Expr::Literal(Literal::Int(text)) = &value {
                return jl::Expr::Literal(Literal::Bool(text != "0"));
            }
        }
        match &type_ {
            jl::Type::Primitive(
                jl::Primitive::String | jl::Primitive::Nothing | jl::Primitive::Any,
            ) => value,
            jl::Type::Primitive(_) | jl::Type::Named(_) => jl::Expr::convert(type_, value),
            // No conversion has a collection or a union target in this
            // walk; the value stands as it is rather than inventing a
            // constructor call nothing defines.
            _ => value,
        }
    }

    /// The value, unchanged: Julia passes every argument by reference
    /// to the same object and takes no address.
    fn handoff(value: Self::Expr, _form: Handoff) -> Self::Expr {
        value
    }

    // -- statements ----------------------------------------------------

    /// `local name::T = value`.
    ///
    /// Annotated because the annotation is load-bearing twice over: it
    /// says the width the value keeps — `local zd_n::UInt32` — and it
    /// converts a later re-assignment, which is what lets `zd_wire =
    /// <literal>` land in the `Int64` the holder declared.
    fn declare(type_: Self::Type, name: &str, value: Option<Self::Expr>) -> Self::Stmt {
        jl::Statement::Local {
            name: Ident::new(name),
            type_: Some(type_),
            value,
        }
    }

    /// An expression evaluated for its effect — or, where the walk
    /// built an assignment, the assignment.
    ///
    /// The shared walk's `assign` helper builds `T::binary("=", …)` and
    /// hands it here, and this is the only place one can arrive. Julia
    /// would accept `a = b` as an expression, and it is still lifted
    /// into a statement: an `=` buried inside a larger expression is an
    /// assignment nobody reads.
    fn perform(value: Self::Expr) -> Self::Stmt {
        if let jl::Expr::Binary { op: "=", lhs, rhs } = value {
            return jl::Statement::Assign {
                place: *lhs,
                value: *rhs,
            };
        }
        jl::Statement::Expr(value)
    }

    /// `return zd_freeze(zd_rt, v)`.
    ///
    /// Every `T::give_back` the decode walk builds is `give_back(None)`
    /// — a `void` decoder in the walk's home column. This column's
    /// decoders answer with the immutable sample, so every return goes
    /// through the freeze; `zd_rt` is the local
    /// `Julia::reading_body` bound to the sample's type, because this
    /// method has no type parameter of its own.
    fn give_back(value: Option<Self::Expr>) -> Self::Stmt {
        jl::Statement::Return(Some(value.unwrap_or_else(|| {
            jl::Expr::free_call(
                "zd_freeze",
                vec![jl::Expr::name("zd_rt"), jl::Expr::name("v")],
            )
        })))
    }

    fn when(
        condition: Self::Expr,
        then: Self::Block,
        otherwise: Option<Self::Block>,
    ) -> Self::Stmt {
        jl::Statement::If {
            condition,
            then: Self::suite(then),
            // An `else` the walk built empty is dropped rather than
            // kept: `else` above `end` says nothing and reads as though
            // a branch had been forgotten.
            otherwise: otherwise.filter(|body| !body.is_empty()),
        }
    }

    /// `for counter in 0:(Int64(bound) - 1)`.
    ///
    /// Through `Int64` first, unconditionally: with an unsigned bound
    /// of zero, `0:(n - 1)` wraps to `0:0xffffffff` and the loop runs
    /// four billion times over a stream that said "none".
    fn count_up(counter: &str, bound: Self::Expr, body: Self::Block) -> Self::Stmt {
        jl::Statement::For {
            counter: Ident::new(counter),
            iter: jl::Expr::range(
                jl::Expr::int(0),
                jl::Expr::Paren(Box::new(jl::Expr::Binary {
                    op: "-",
                    lhs: Box::new(jl::Expr::free_call("Int64", vec![bound])),
                    rhs: Box::new(jl::Expr::int(1)),
                })),
            ),
            body: Self::suite(body),
        }
    }

    fn until(condition: Self::Expr, body: Self::Block) -> Self::Stmt {
        jl::Statement::While {
            condition,
            body: Self::suite(body),
        }
    }

    fn leave() -> Self::Stmt {
        jl::Statement::Break
    }

    fn note(doc: Doc) -> Self::Stmt {
        jl::Statement::Comment(doc)
    }

    /// The call, and nothing around it: Julia has no discarded-result
    /// diagnostic and `(Nothing) expr` is not a statement.
    fn drop_value(value: Self::Expr) -> Self::Stmt {
        jl::Statement::Expr(value)
    }

    /// Nothing: Julia does not warn on an unread parameter, and a bare
    /// `zd_tc` on a line of its own would be a statement that does
    /// nothing.
    fn mark_used(_name: &str) -> Vec<Self::Stmt> {
        Vec::new()
    }

    /// `place = zd_dec_T(r, zd_tc)`.
    ///
    /// A Julia function has no out-parameter — rebinding a parameter
    /// rebinds the local — so this column's decoders answer with the
    /// value and the destination is assigned at the call site.
    fn read_call(
        callee: &str,
        place: Self::Expr,
        _form: Handoff,
        behaviour: Self::Expr,
    ) -> Self::Stmt {
        jl::Statement::Assign {
            place,
            value: jl::Expr::free_call(callee, vec![jl::Expr::name("r"), behaviour]),
        }
    }

    /// `v = zd_dec_T(r, zd_tc)` — a `typedef`, handed straight on. The
    /// nested decoder answers a frozen value and the trailing freeze is
    /// then the identity.
    fn read_forward(callee: &str) -> Self::Stmt {
        jl::Statement::Assign {
            place: jl::Expr::name("v"),
            value: jl::Expr::free_call(callee, vec![jl::Expr::name("r"), jl::Expr::name("zd_tc")]),
        }
    }

    fn block(statements: Vec<Self::Stmt>) -> Self::Block {
        statements
    }

    fn into_statements(block: Self::Block) -> Vec<Self::Stmt> {
        block
    }

    /// Room for `count` values, and the fill inside it.
    ///
    /// **A sequence** (`assigned_whole`): `v = zd_nothing_vector(n)`,
    /// a `Vector{Any}` of `nothing`, because the element decoder
    /// assigns each slot outright and `zd_freeze` converts the run to
    /// the declared `Vector{T}` or `NTuple` at the return.
    ///
    /// **A `map`** (not `assigned_whole`): `v = zd_entry_vector(n)`,
    /// one *distinct* `ZdScratch` per entry, because each entry is
    /// written half at a time — `v[i].key = …` — and a repeated
    /// reference would make every entry the last one read.
    fn make_room(
        base: &str,
        _element: Self::Type,
        count: Self::Expr,
        assigned_whole: bool,
        fill: Vec<Self::Stmt>,
    ) -> Vec<Self::Stmt> {
        let maker = if assigned_whole {
            "zd_nothing_vector"
        } else {
            "zd_entry_vector"
        };
        let mut out = vec![jl::Statement::Assign {
            place: jl::Expr::name(base),
            value: jl::Expr::free_call(maker, vec![count]),
        }];
        out.extend(fill);
        out
    }

    /// `v = v[1:Int64(count)]`.
    ///
    /// §7.3.1.2.1.12.3's TRIM for a bounded collection, applied after
    /// every element has been read. One-based and inclusive, which is
    /// what "the first `count` elements" is in Julia's own counting.
    fn keep_first(base: &str, count: Self::Expr) -> Vec<Self::Stmt> {
        vec![jl::Statement::Assign {
            place: jl::Expr::name(base),
            value: jl::Expr::index(
                jl::Expr::name(base),
                jl::Expr::range(jl::Expr::int(1), jl::Expr::free_call("Int64", vec![count])),
            ),
        }]
    }

    /// The subscript, with nothing before or after it: a `Vector{Any}`
    /// slot is an ordinary place at every element type.
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

    /// A local the body writes *into*.
    ///
    /// One caller: `zd_scratch`, the entry of an uncounted `map`, which
    /// the reader fills a half at a time — a runtime `ZdScratch`, for
    /// the reason `MAP_ENTRY` gives.
    fn storage(type_: Self::Type, name: &str) -> Self::Stmt {
        if type_ == jl::Type::named(MAP_ENTRY) {
            return jl::Statement::Assign {
                place: jl::Expr::name(name),
                value: jl::Expr::free_call("zd_scratch_entry", Vec::new()),
            };
        }
        Self::declare(type_, name, None)
    }

    /// One field of a header the runtime handed back.
    ///
    /// `end` is a Julia keyword and cannot name a struct field, so the
    /// runtime's `ZdPidRead` spells it `end_` and this seam is where
    /// the walk's name meets the runtime's.
    fn header_field(value: Self::Expr, field: &str) -> Self::Expr {
        let spelled = if field == "end" { "end_" } else { field };
        jl::Expr::field(value, spelled)
    }

    // -- assembly ------------------------------------------------------

    /// `function zd_enc_T(w::ZdWriter, v::T) … end` and
    /// `function zd_dec_T(r::ZdReader, zd_tc::ZdTryConstruct) … end`.
    ///
    /// A forward declaration is answered with nothing: Julia resolves a
    /// global at the call and not at the definition, so a recursive or
    /// mutually recursive type needs no declaration ahead of its body.
    fn routine(spec: Routine<Self>) -> Option<Self::Decl> {
        let body = spec.body?;
        let mut params = vec![jl::Param {
            name: Ident::new(if spec.writing { "w" } else { "r" }),
            type_: Some(jl::Type::named(if spec.writing {
                "ZdWriter"
            } else {
                "ZdReader"
            })),
        }];
        if spec.writing {
            params.push(jl::Param {
                name: Ident::new("v"),
                // Unannotated on purpose. The declared sample type
                // would be right for every call the walk builds, and
                // one caller sits outside the walk: the write harness
                // hands the sample it froze, and an annotation here
                // would re-convert a value that is already exact.
                type_: None,
            });
        }
        params.extend(spec.extra.into_iter().map(|(type_, name)| jl::Param {
            name: Ident::new(&name),
            type_: Some(type_),
        }));
        let body = if spec.writing {
            Self::suite(body)
        } else {
            Self::reading_body(&spec.name, spec.sample, body)
        };
        Some(jl::Decl::FunctionDef(jl::FunctionDef {
            doc: spec.doc,
            name: Ident::new(&spec.name),
            params,
            // No return annotation: a reader's returns already go
            // through `zd_freeze`, and an annotation would ask Julia to
            // convert a value that is already the declared type.
            returns: None,
            body,
        }))
    }

    /// A call to a generated defaulter and not a literal in place;
    /// [`crate::default_julia`] carries the whole table.
    ///
    /// # Errors
    /// Never; the signature matches the emitters around it.
    fn default_into(
        ast: &Ast,
        id: Id<TypeNode>,
        place: Self::Expr,
    ) -> Result<Self::Stmt, EncodeError> {
        Ok(crate::default_julia::default_into(ast, id, place))
    }

    /// # Errors
    /// As [`crate::default_julia::member_default`]: a `@default` this
    /// column cannot spell as a Julia expression is refused rather than
    /// silently replaced by Table 9's zero.
    fn member_default(
        ast: &Ast,
        node: &MemberNode,
        type_: Id<TypeNode>,
        place: Self::Expr,
    ) -> Result<Vec<Self::Stmt>, EncodeError> {
        crate::default_julia::member_default(ast, node, type_, place)
    }

    /// `a_b_RED` — the flattened constant `adapter/julia.rs` binds an
    /// enumerator to at module scope.
    ///
    /// Through `escape_name` and not [`Self::escape`]: an enumerator is
    /// a module-scope binding and shadows what a field cannot, so the
    /// two escapes are different sets and the materialiser used the
    /// wider one.
    fn enumerator_constant(ast: &Ast, id: Id<TypeNode>, enumerator: &str) -> Self::Expr {
        jl::Expr::name(&format!(
            "{}{}",
            julia_module_prefix(ast, id),
            escape_name(enumerator)
        ))
    }
}

/// A code unit read as C's signed `char`.
///
/// `((Int64(k) + 128) % 256) - 128`: the two's-complement reading of one
/// octet, for `MapKeyOrdering::SignedInteger` over an IDL `char` key.
/// The corpus was recorded on a host where C's `char` is signed, so
/// `U+00FF` sorts below `U+0000`, and a column comparing unsigned code
/// units would order the entries differently.
///
/// Arithmetic and not a helper in `runtime/julia`, for the reason the
/// Python column writes down: how wide and how signed C spells a
/// character is a language-mapping question, and a runtime whose subject
/// is §7.4 is not where a second column should inherit this column's
/// host from.
fn signed_char(value: jl::Expr) -> jl::Expr {
    let wide = jl::Expr::free_call("Int64", vec![value]);
    let wrapped = jl::Expr::Binary {
        op: "%",
        lhs: Box::new(jl::Expr::Paren(Box::new(jl::Expr::Binary {
            op: "+",
            lhs: Box::new(wide),
            rhs: Box::new(jl::Expr::int(128)),
        }))),
        rhs: Box::new(jl::Expr::int(256)),
    };
    jl::Expr::Binary {
        op: "-",
        lhs: Box::new(jl::Expr::Paren(Box::new(wrapped))),
        rhs: Box::new(jl::Expr::int(128)),
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
/// generated Julia held no `zd_over` check at all, so `a` accepted a
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

/// The name component a codec function takes for a type.
///
/// The IDL kind and never the Julia spelling: `octet` and `char` are one
/// `UInt8` here and one octet apart on no wire, but `double` and every
/// integer width map one to one — the tag keeps them apart the same way
/// the Python column's does. An alias is followed through, so a typedef
/// and its target share one codec and the walk's first-wins
/// deduplication keeps the target's body.
///
/// zerodds-lint: recursion-depth 64 (collection nesting; bounded by `MAX_TYPE_DEPTH`)
fn codec_tag(ast: &Ast, id: Id<TypeNode>, depth: usize) -> String {
    if depth > MAX_TYPE_DEPTH {
        return "deep".to_string();
    }
    let Ok(node) = ast.type_(id) else {
        return "unknown".to_string();
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
                codec_tag(ast, element, depth + 1)
            ),
            None => format!("seq_unknown{}", bound_tag(ast, bound)),
        },
        TypeKind::Array { element, dims } => {
            let mut name = match element.resolved() {
                Some(element) => format!("arr_{}", codec_tag(ast, element, depth + 1)),
                None => "arr_unknown".to_string(),
            };
            for dim in dims {
                match ast.eval_integral(*dim) {
                    Ok(length) => name.push_str(&format!("_{length}")),
                    // No number rather than a zero: the name has to
                    // differ from a `[0]` array's.
                    Err(_) => name.push_str("_x"),
                }
            }
            name
        }
        // The entry struct's own name carries no bound — two maps that
        // differ only in bound hold the same pairs — but the tag does,
        // for the routine-name reason in [`bound_tag`].
        TypeKind::Map { bound, .. } => {
            {
                // The collection's bound goes in front of the entry name:
                // after it, a bound on the map and one on its value would
                // end in the same suffix.
                let entry = julia_map_entry_name(ast, id);
                match bound_tag(ast, bound) {
                    bound if bound.is_empty() => entry,
                    bound => format!("map{bound}_{entry}"),
                }
            }
        }
        TypeKind::Alias { aliased } => match aliased.resolved() {
            Some(target) => codec_tag(ast, target, depth + 1),
            None => julia_type_name(ast, id),
        },
        _ => julia_type_name(ast, id),
    }
}
