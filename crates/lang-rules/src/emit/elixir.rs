// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Elixir backend of the shared codec walk.
//!
//! DDS-XTypes 1.3 §7.4 from the clause, the same as `runtime/c`,
//! `runtime/python` and the rest do, and none of them calls another.
//! Where two columns disagree on an octet, one has misread the
//! specification — which is the only reason to have a tenth.
//!
//! # The decision this column turns on
//!
//! **Elixir has no mutation, and the walk assumes one.** The shared walk
//! rebinds locals inside loop bodies and `if` branches — `w` after every
//! write, `v` after every member — and expects the binding to survive
//! the block, which in Elixir it does not. This backend answers by
//! building the statement layer of `zerodds_lang_cst::elixir`, where
//! every binding is *named on the statement*, and
//! `zerodds_lang_render::elixir::codec_function` threads the changed
//! bindings out of each block as an accumulator tuple. The division of
//! labour is deliberate: this file decides *which* statement a walk
//! operation is, the renderer decides how a block hands its rebindings
//! out, and neither answers the other's question.
//!
//! Concretely, every runtime operation is classified once, in
//! `writer_op` and `reader_op`:
//!
//! - a **pure** operation (`reader_position`, `bitfield`, the trims)
//!   stays an expression: `ZeroddsWire.Reader.position(r)`;
//! - a **state-changing** operation with no result rebinds its
//!   receiver: `w = ZeroddsWire.Writer.write_u32(w, x)`;
//! - one with a result binds both: `{zd_rv, r} =
//!   ZeroddsWire.Reader.read_u32(r)` — and the value the walk wanted is
//!   `zd_rv`, hoisted ahead of the expression it sat in.
//!
//! A state-changing call that survives into a position this file cannot
//! hoist from — an `until` condition, which is re-evaluated per pass —
//! is a panic naming the operation, never a call quietly evaluated
//! once: the three conditions the walks build are pure, and a fourth
//! that is not must fail the generator rather than ship a loop that
//! stops early.
//!
//! # Two smaller decisions
//!
//! **An integer is unbounded, so a width is never a cast.** The same
//! ground the Python column stands on: [`CodecTarget::cast`] is the
//! identity on every integer, the runtime range-checks at the octet,
//! and the width lives in the codec's *name* through `type_tag`.
//!
//! **A `map` entry is a two-element list, both directions.** The
//! materialiser's `@type` writes `list({K, V})`, but a tuple cannot be
//! written into half at a time, which is exactly what the shared read
//! walk does — `entry[0]`, then `entry[1]`. The probe's own filler
//! builds `[k, v]` lists and its dumper reads them, so the codec never
//! meets a tuple; `ZeroddsWire.entry_half/2` accepts both so a
//! hand-built sample that used tuples is read rather than crashed on.

use super::{CodecTarget, Constant, Handle, Handoff, Routine, Scalar};
use crate::adapter::elixir::type_module_path;
use crate::encode::EncodeError;
use zerodds_ast::{Ast, CharWidth, MemberNode, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::{Doc, Ident, Literal, Path};
use zerodds_lang_cst::elixir::{self as ex, atom_name};

/// The Elixir backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Elixir;

/// How deep a chain of typedefs and collections is followed.
const MAX_TYPE_DEPTH: usize = 64;

/// The type name [`CodecTarget::named`] is handed for C's signed
/// `char`; see the Python backend, which this mirrors.
const SIGNED_CHAR: &str = "signed char";

/// What [`CodecTarget::map_entry_name`] answers with — a marker the
/// seams that receive it match on, because a `map` entry has no Elixir
/// module and both need to know the thing they are making room for is a
/// two-slot list.
const MAP_ENTRY: &str = "_zd_map_entry";

/// The marker segment a writer operation carries until a statement seam
/// resolves it. Never rendered: everything that can hold an expression
/// goes through [`lower`] first, and `lower` panics on a marker it
/// cannot place.
const W_MARK: &str = "ZdW";
/// The reader counterpart of [`W_MARK`].
const R_MARK: &str = "ZdR";

thread_local! {
    /// The number the next temporary of the routine being built takes.
    /// See [`Elixir::fresh`] and [`Elixir::reset_temporaries`].
    static NEXT_TEMPORARY: core::cell::Cell<u64> = const { core::cell::Cell::new(0) };
}

/// What a writer operation does to the writer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OpShape {
    /// No state change: an ordinary expression.
    Pure,
    /// Rebinds the receiver, answers nothing.
    Mutates,
    /// Rebinds the receiver *and* answers a value.
    MutatesWithValue,
}

/// The shape of one writer operation.
fn writer_op(method: &str) -> OpShape {
    match method {
        "bitfield" => OpShape::Pure,
        "write_dheader"
        | "write_emheader"
        | "write_pid"
        | "write_pid_extended"
        | "begin_packed_element" => OpShape::MutatesWithValue,
        _ => OpShape::Mutates,
    }
}

/// The shape of one reader operation.
fn reader_op(method: &str) -> OpShape {
    match method {
        "reader_status" | "reader_position" | "reader_remaining" | "reader_save" | "trim_utf8"
        | "trim_utf16" => OpShape::Pure,
        "reader_refuse"
        | "reader_restore"
        | "reader_skip_to"
        | "finish_pid_read"
        | "reader_finish_packed_element" => OpShape::Mutates,
        // Every `read_*`, and `reader_clear_discard`.
        _ => OpShape::MutatesWithValue,
    }
}

impl Elixir {
    /// The runtime module a marked call resolves to.
    fn runtime_path(marker: &str, method: &str) -> Path {
        let module = if marker == W_MARK { "Writer" } else { "Reader" };
        Path::new(&["ZeroddsWire", module, method])
    }

    /// The receiver a marked call threads.
    fn receiver(marker: &str) -> &'static str {
        if marker == W_MARK { "w" } else { "r" }
    }

    /// A temporary's name, unique within the routine being built.
    ///
    /// Unique rather than rebound: a temporary reused across scopes is
    /// a name bound before an `if` and rebound inside it, which the
    /// renderer's threading would then carry out of the block — and the
    /// carried binding, never read again, is a warning, and warnings
    /// are failures in this column's compile gates.
    ///
    /// The counter is thread-local and [`Self::routine`] returns it to
    /// zero as it closes each routine, so the same input produces the
    /// same text: a routine is one Elixir function and one scope, which
    /// is as far as a temporary's name has to reach, and a counter that
    /// ran on past the end of a routine would number the next one
    /// differently depending on what else this process had generated
    /// first — two types in one run, or two tests on one thread.
    fn fresh(prefix: &str) -> Ident {
        let number = NEXT_TEMPORARY.with(|next| {
            let number = next.get();
            next.set(number.wrapping_add(1));
            number
        });
        Ident::new(&format!("{prefix}{number}"))
    }

    /// The temporary counter back to zero, at the end of a routine.
    fn reset_temporaries() {
        NEXT_TEMPORARY.with(|next| next.set(0));
    }

    /// Hoist every state-changing runtime call out of `expr`.
    ///
    /// Answers the statements that have to run first and the residual
    /// expression, which is pure.
    fn lower(expr: ex::Expr) -> (Vec<ex::Statement>, ex::Expr) {
        let mut statements = Vec::new();
        let mut temporaries = 0usize;
        let value = Self::lower_into(expr, &mut statements, &mut temporaries);
        (statements, value)
    }

    fn lower_into(
        expr: ex::Expr,
        statements: &mut Vec<ex::Statement>,
        temporaries: &mut usize,
    ) -> ex::Expr {
        match expr {
            ex::Expr::Call { func, args } => {
                let marker = func
                    .segments()
                    .first()
                    .map(|segment| segment.as_str().to_string());
                let args: Vec<ex::Expr> = args
                    .into_iter()
                    .map(|argument| Self::lower_into(argument, statements, temporaries))
                    .collect();
                let Some(marker) = marker.filter(|m| m == W_MARK || m == R_MARK) else {
                    return ex::Expr::Call { func, args };
                };
                let method = func
                    .segments()
                    .get(1)
                    .map(Ident::as_str)
                    .unwrap_or_default()
                    .to_string();
                let shape = if marker == W_MARK {
                    writer_op(&method)
                } else {
                    reader_op(&method)
                };
                let receiver = Self::receiver(&marker);
                let mut full_args = vec![ex::Expr::Var(Ident::new(receiver))];
                full_args.extend(args);
                let call = ex::Expr::Call {
                    func: Self::runtime_path(&marker, &method),
                    args: full_args,
                };
                match shape {
                    OpShape::Pure => call,
                    // A state change with no value in expression
                    // position is a walk shape this backend has no
                    // lowering for. The two statement seams
                    // ([`Elixir::perform`], [`Elixir::drop_value`])
                    // intercept it first, so reaching this is a walk
                    // that changed underneath the backend — and the
                    // no-panic contract turns the refusal into a call
                    // to a name no module defines: `elixirc` stops on
                    // it and names the operation, which is louder than
                    // a call quietly evaluated for a value it does not
                    // have.
                    OpShape::Mutates => ex::Expr::Call {
                        func: Path::single(&format!("zd_unlowered_{method}")),
                        args: Vec::new(),
                    },
                    OpShape::MutatesWithValue => {
                        *temporaries += 1;
                        let name = Self::fresh("zd_rv");
                        statements.push(ex::Statement::BindTuple {
                            names: vec![name.clone(), Ident::new(receiver)],
                            value: call,
                        });
                        ex::Expr::Var(name)
                    }
                }
            }
            ex::Expr::Binary { op, lhs, rhs } => {
                let lhs = Self::lower_into(*lhs, statements, temporaries);
                let rhs = Self::lower_into(*rhs, statements, temporaries);
                ex::Expr::Binary {
                    op,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                }
            }
            ex::Expr::Unary { op, value } => ex::Expr::Unary {
                op,
                value: Box::new(Self::lower_into(*value, statements, temporaries)),
            },
            ex::Expr::Paren(value) => {
                ex::Expr::Paren(Box::new(Self::lower_into(*value, statements, temporaries)))
            }
            ex::Expr::Field { value, name } => ex::Expr::Field {
                value: Box::new(Self::lower_into(*value, statements, temporaries)),
                name,
            },
            ex::Expr::Index { value, index } => ex::Expr::Index {
                value: Box::new(Self::lower_into(*value, statements, temporaries)),
                index: Box::new(Self::lower_into(*index, statements, temporaries)),
            },
            ex::Expr::IfElse {
                condition,
                then,
                otherwise,
            } => ex::Expr::IfElse {
                condition: Box::new(Self::lower_into(*condition, statements, temporaries)),
                then: Box::new(Self::lower_into(*then, statements, temporaries)),
                otherwise: Box::new(Self::lower_into(*otherwise, statements, temporaries)),
            },
            other @ (ex::Expr::Literal(_)
            | ex::Expr::Atom(_)
            | ex::Expr::Var(_)
            | ex::Expr::Attr(_)
            | ex::Expr::Module(_)
            | ex::Expr::List(_)
            | ex::Expr::Tuple(_)
            | ex::Expr::Keyword(_)
            | ex::Expr::StructLiteral { .. }) => other,
        }
    }

    /// Where a value lands: the base variable and the access path from
    /// it. `None` for an expression the walk never assigns into.
    fn place_of(place: &ex::Expr) -> Option<(Ident, Vec<ex::Access>)> {
        match place {
            ex::Expr::Var(name) => Some((name.clone(), Vec::new())),
            ex::Expr::Field { value, name } => {
                let (base, mut path) = Self::place_of(value)?;
                path.push(ex::Access::Key(name.clone()));
                Some((base, path))
            }
            ex::Expr::Index { value, index } => {
                let (base, mut path) = Self::place_of(value)?;
                path.push(ex::Access::At((**index).clone()));
                Some((base, path))
            }
            _ => None,
        }
    }

    /// The statement a place this backend cannot decompose becomes: a
    /// call to a name no module defines, so `elixirc` stops on the
    /// generated file and the value is never quietly dropped.
    fn refused_place(place: ex::Expr, value: ex::Expr) -> ex::Statement {
        ex::Statement::Bind {
            name: Ident::new("_"),
            value: ex::Expr::Call {
                func: Path::single("zd_refused_not_a_place"),
                args: vec![place, value],
            },
        }
    }

    /// `place = value`, as the statements that get it there.
    fn assign(place: ex::Expr, value: ex::Expr) -> Vec<ex::Statement> {
        let (mut statements, value) = Self::lower(value);
        let Some((base, path)) = Self::place_of(&place) else {
            statements.push(Self::refused_place(place, value));
            return statements;
        };
        if path.is_empty() {
            statements.push(ex::Statement::Bind { name: base, value });
        } else {
            statements.push(ex::Statement::Update { base, path, value });
        }
        statements
    }

    /// One statement out of several, flattened later by the renderer.
    fn seq(mut statements: Vec<ex::Statement>) -> ex::Statement {
        if statements.len() == 1 {
            return statements.remove(0);
        }
        ex::Statement::Seq(statements)
    }

    /// Whether a call is to a generated *encoder*, which threads the
    /// writer: `w = zd_enc_T(w, value)`.
    fn is_encoder_call(func: &Path) -> bool {
        func.segments().len() == 1
            && func.segments().first().is_some_and(|name| {
                let name = name.as_str();
                name.starts_with("zd_enc_")
                    || name.starts_with("zd_kenc_")
                    || name.starts_with("zd_keyholder_")
            })
    }
}

impl CodecTarget for Elixir {
    type Expr = ex::Expr;
    type Stmt = ex::Statement;
    type Block = Vec<ex::Statement>;
    type Type = ex::Type;
    type Decl = ex::CodecFunction;

    // -- types --------------------------------------------------------

    /// One `integer()` for every integer width, because Elixir has one.
    ///
    /// A `char` and a `wchar` are marked with their own tag: both are a
    /// one-character binary at run time, and [`Self::cast`] needs to
    /// tell a character place from a string one to rewrite a union
    /// label into the right literal.
    fn scalar(kind: Scalar) -> Self::Type {
        match kind {
            Scalar::Bool => ex::Type::Basic(ex::Basic::Boolean),
            Scalar::Char => ex::Type::Atom("char".to_string()),
            Scalar::Float | Scalar::Double => ex::Type::Basic(ex::Basic::Float),
            Scalar::LongDouble => ex::Type::Atom("float128".to_string()),
            Scalar::Int8
            | Scalar::UInt8
            | Scalar::Int16
            | Scalar::UInt16
            | Scalar::Int32
            | Scalar::UInt32
            | Scalar::Int64
            | Scalar::UInt64
            | Scalar::Index
            | Scalar::Size => ex::Type::Basic(ex::Basic::Integer),
        }
    }

    fn named(name: &str) -> Self::Type {
        ex::Type::Atom(name.to_string())
    }

    fn nothing() -> Self::Type {
        ex::Type::Basic(ex::Basic::Nil)
    }

    /// The runtime's handles are maps and need no declared type; the
    /// tag exists so a declaration without an initialiser can be told
    /// apart in a debugger of the generator, not of the generated code.
    fn handle(kind: Handle) -> Self::Type {
        ex::Type::Atom(
            match kind {
                Handle::DHeader => "dheader",
                Handle::EmHeader => "emheader",
                Handle::Pid => "pid",
                Handle::TryConstruct => "try_construct",
                Handle::ReaderMark => "reader_mark",
                Handle::PidRead => "pid_read",
                Handle::EmHeaderRead => "emheader_read",
            }
            .to_string(),
        )
    }

    /// A one-character binary is what a `char` is here; the runtime
    /// takes it whole and refuses what the type cannot hold.
    fn code_point(value: Self::Expr, _narrow: bool) -> Self::Expr {
        value
    }

    /// The runtime's policy values, as the atoms
    /// `runtime/elixir` matches on.
    ///
    /// Atoms rather than strings because that is what an Elixir function
    /// head matches, and the quoted forms — `:"character-count"` — keep
    /// the exact spellings of `crates/ast-layout/src/key.rs`, so the
    /// measurement's names survive into the generated file. One arm per
    /// constant and no catch-all, for the reason every backend states.
    fn constant(which: Constant) -> Self::Expr {
        let atom = |name: &str| ex::Expr::Atom(name.to_string());
        match which {
            Constant::WcharWidth2 => ex::Expr::int(2),
            Constant::WcharWidth4 => ex::Expr::int(4),
            Constant::WcharFixed => atom("fixed"),
            Constant::WcharGiopLengthPrefixed => atom("giop-length-prefixed"),
            Constant::WstringUnitsStream => atom("stream"),
            Constant::WstringUnitsBigEndianAlways => atom("big-endian-always"),
            Constant::WstringUnitsByteOrderMark => atom("byte-order-mark"),
            Constant::WstringUnitsHostUnswapped => atom("host-unswapped"),
            Constant::WstringSpecification => atom("specification"),
            Constant::WstringCharacterCount => atom("character-count"),
            Constant::WstringCharacterCountTerminated => atom("character-count-terminated"),
            Constant::WstringSpecificationWideUnits => atom("specification-wide-units"),
            Constant::WstringRosTypeSupport => atom("ros-type-support"),
            Constant::WstringOctetsTerminated2 => atom("octets/terminated/2"),
            Constant::WstringOctetsTerminated4 => atom("octets/terminated/4"),
            Constant::WstringCharactersTerminated2 => atom("characters/terminated/2"),
            Constant::FixedDeclared => atom("declared"),
            Constant::FixedValuePrecision => atom("value-precision"),
            Constant::FixedValueStringDigits => atom("value-string-digits"),
            Constant::ParameterMember => atom("member"),
            Constant::ParameterOptional => atom("optional"),
            Constant::TryConstructDiscard => atom("discard"),
            Constant::TryConstructTrim => atom("trim"),
            Constant::TryConstructUseDefault => atom("use_default"),
            Constant::StatusOk => atom("ok"),
            Constant::StatusDiscarded => atom("discarded"),
            Constant::StatusImpossibleLength => atom("impossible_length"),
            Constant::StatusOutOfRange => atom("out_of_range"),
            Constant::StatusTruncated => atom("truncated"),
        }
    }

    /// `integer()`, at every bit bound.
    fn unsigned_holder(_bits: u16) -> Self::Type {
        ex::Type::Basic(ex::Basic::Integer)
    }

    fn value_type(ast: &Ast, id: Id<TypeNode>) -> Self::Type {
        value_type_of(ast, id, 0)
    }

    fn type_name(ast: &Ast, id: Id<TypeNode>) -> String {
        type_tag(ast, id, 0)
    }

    /// A `map`'s entry is a two-element list and has no module; see the
    /// module documentation.
    fn map_entry_name(_ast: &Ast, _id: Id<TypeNode>) -> String {
        MAP_ENTRY.to_string()
    }

    /// Nothing: an Elixir module name carries its whole scope and every
    /// reference is written from the root.
    fn module_prefix(_ast: &Ast, _id: Id<TypeNode>) -> String {
        String::new()
    }

    fn escape(name: &str) -> String {
        atom_name(name)
    }

    // -- expressions ---------------------------------------------------

    fn name(text: &str) -> Self::Expr {
        ex::Expr::Var(Ident::new(text))
    }

    fn int(value: i128) -> Self::Expr {
        ex::Expr::int(value)
    }

    /// The number: an Elixir integer is unbounded and needs no suffix.
    fn wide_int(value: i128) -> Self::Expr {
        ex::Expr::int(value)
    }

    /// As [`Self::wide_int`].
    fn wide_unsigned(value: u64) -> Self::Expr {
        ex::Expr::int(i128::from(value))
    }

    /// As [`Self::wide_unsigned`].
    fn wide_signed(value: i128) -> Self::Expr {
        ex::Expr::int(value)
    }

    /// The number, with no suffix: `2u` is not an Elixir token.
    fn unsigned_int(value: i128) -> Self::Expr {
        ex::Expr::int(value)
    }

    /// A literal, with the two raw tokens the walk escapes through
    /// [`Literal::Int`] rewritten — `true` and `false` are the same
    /// spelling in Elixir, but the tree's boolean is what says a
    /// boolean was meant.
    fn literal(value: Literal) -> Self::Expr {
        let Literal::Int(text) = &value else {
            return ex::Expr::Literal(value);
        };
        match text.as_str() {
            "true" => ex::Expr::Literal(Literal::Bool(true)),
            "false" => ex::Expr::Literal(Literal::Bool(false)),
            _ => ex::Expr::Literal(value),
        }
    }

    /// A call to a generated function — or, for `strcmp`, the sign
    /// expression Elixir's own binary comparison gives.
    ///
    /// Erlang compares binaries bytewise, which is `strcmp`'s
    /// `unsigned char` order (C11 §7.24.4), so the sign is two
    /// comparisons and a subtraction.
    fn call(callee: &str, args: Vec<Self::Expr>) -> Self::Expr {
        if callee == "strcmp" {
            return strcmp_sign(args);
        }
        ex::Expr::Call {
            func: Path::single(callee),
            args,
        }
    }

    /// An operation on the runtime, carried as a marked call until a
    /// statement seam places it. See the module documentation.
    fn runtime(receiver: &str, method: &str, args: Vec<Self::Expr>) -> Self::Expr {
        let marker = if receiver == "w" { W_MARK } else { R_MARK };
        ex::Expr::Call {
            func: Path::new(&[marker, method]),
            args,
        }
    }

    /// A free function of the `ZeroddsWire` module; every one the walks
    /// ask for is pure.
    fn runtime_static(method: &str, args: Vec<Self::Expr>) -> Self::Expr {
        ex::Expr::Call {
            func: Path::new(&["ZeroddsWire", method]),
            args,
        }
    }

    /// `runtime/elixir` carries the three HDDS axes:
    /// `begin_packed_element` / `finish_packed_element` on the writer and
    /// `reader_begin_packed_element` / `reader_finish_packed_element` on the
    /// reader, the DHEADER pair for a sequence element's own length, and
    /// `write_wstring_utf8_octets` / `read_wstring_utf8_octets`.
    fn carries_hdds_axes() -> bool {
        true
    }

    /// A field name with dots is a path through the members that hold a base
    /// (`parent.x`, XTypes 1.3 §7.5.1.4.1.3): one field step per segment, which is what
    /// the place of an update decomposes into.
    fn member(base: &str, field: &str) -> Self::Expr {
        let mut place = Self::name(base);
        for segment in field.split('.') {
            place = ex::Expr::Field {
                value: Box::new(place),
                name: Ident::new(segment),
            };
        }
        place
    }

    /// The base is the first member `parent` (XTypes 1.3 §7.5.1.4.1.3).
    fn inherited_path(depth: usize) -> Option<String> {
        Some(format!("{}.", atom_name("parent")).repeat(depth))
    }

    fn whole(base: &str) -> Self::Expr {
        Self::name(base)
    }

    fn field_of(value: Self::Expr, field: &str) -> Self::Expr {
        ex::Expr::Field {
            value: Box::new(value),
            name: Ident::new(field),
        }
    }

    /// The runtime's headers are plain maps, but two of their field
    /// names are not Elixir's to take: `end` is a reserved word and
    /// does not parse after a dot, so the reader publishes the
    /// sentinel flag as `list_end`.
    fn header_field(value: Self::Expr, field: &str) -> Self::Expr {
        let named = if field == "end" { "list_end" } else { field };
        Self::field_of(value, named)
    }

    fn index(value: Self::Expr, at: Self::Expr) -> Self::Expr {
        ex::Expr::Index {
            value: Box::new(value),
            index: Box::new(at),
        }
    }

    /// `length(v)`, or the bound where the materialiser fixed one.
    fn count_of(base: &str, fixed: Option<u64>) -> Self::Expr {
        match fixed {
            Some(bound) => ex::Expr::int(i128::from(bound)),
            None => ex::Expr::Call {
                func: Path::single("length"),
                args: vec![Self::name(base)],
            },
        }
    }

    fn elements(base: &str) -> Self::Expr {
        Self::name(base)
    }

    /// `v.field` — a union's case member is a plain struct field, the
    /// materialiser's one-struct union shape.
    fn case_member(base: &str, field: &str) -> Self::Expr {
        Self::member(base, field)
    }

    /// `ZeroddsWire.entry_half(entry, 0 | 1)` on the way *out* would be
    /// unassignable, so both halves are the index form and the write
    /// side rebuilds through `Access.at`. See the module documentation.
    fn entry_half(entry: Self::Expr, key: bool) -> Self::Expr {
        ex::Expr::Index {
            value: Box::new(entry),
            index: Box::new(ex::Expr::int(i128::from(!key))),
        }
    }

    fn absent() -> Self::Expr {
        ex::Expr::nil()
    }

    /// Nothing: an Elixir binary owns its storage and a read either
    /// produced one or raised.
    fn materialised(_value: Self::Expr) -> Option<Self::Expr> {
        None
    }

    /// The member itself: absence is `nil` and the value needs no
    /// unwrapping.
    fn optional_value(_ast: &Ast, _id: Id<TypeNode>, base: &str, field: &str) -> Self::Expr {
        Self::member(base, field)
    }

    /// `v.field != nil`.
    ///
    /// `!=` and not `!==`: the compared value is `nil` and nothing else
    /// compares equal to it, so the strict form would claim a
    /// distinction that does not exist here.
    fn present(base: &str, field: &str) -> Self::Expr {
        ex::Expr::Binary {
            op: "!=",
            lhs: Box::new(Self::member(base, field)),
            rhs: Box::new(Self::absent()),
        }
    }

    /// `v = ZeroddsWire.array_to_lists(v, depth)`: the tuple
    /// `default_elixir.rs` answers for an array, as the nested lists
    /// `put_in(v, [Access.at(..)], ..)` writes into — `Access.at/1`
    /// refuses a tuple.
    fn open_array(base: &str, lengths: &[i128]) -> Vec<Self::Stmt> {
        vec![ex::Statement::Bind {
            name: Ident::new(base),
            value: Self::runtime_static(
                "array_to_lists",
                vec![Self::whole(base), ex::Expr::int(lengths.len() as i128)],
            ),
        }]
    }

    /// `v = ZeroddsWire.array_to_tuples(v, depth)`: the rows back as
    /// the `{T, …}` the materialiser declares the member as. A list in
    /// its place is not `==` to the tuple a caller built (measured:
    /// `%A{w: [1, 2, 3]} == %A{w: {1, 2, 3}}` was `false` for a decoded
    /// `long w[3]`).
    fn close_array(base: &str, lengths: &[i128]) -> Vec<Self::Stmt> {
        vec![ex::Statement::Bind {
            name: Ident::new(base),
            value: Self::runtime_static(
                "array_to_tuples",
                vec![Self::whole(base), ex::Expr::int(lengths.len() as i128)],
            ),
        }]
    }

    /// Nothing on the way in — writing the member is what makes it
    /// present — and `v = %{v | field: nil}` on the way out.
    fn set_present(base: &str, field: &str, present: bool) -> Vec<Self::Stmt> {
        if present {
            return Vec::new();
        }
        vec![ex::Statement::Update {
            base: Ident::new(base),
            path: vec![ex::Access::Key(Ident::new(field))],
            value: Self::absent(),
        }]
    }

    /// `v = %{v | field: nil}` for every case: a union is a struct with
    /// one nullable field per branch, and only the selected one holds a
    /// value. The same statement as an absent
    /// `@optional` member, [`Self::set_present`] with `false`.
    fn clear_union_cases(base: &str, fields: &[String]) -> Vec<Self::Stmt> {
        fields
            .iter()
            .flat_map(|field| Self::set_present(base, field, false))
            .collect()
    }

    /// The member, with nothing to do first: a struct field holding
    /// `nil` is already somewhere to write.
    fn optional_destination(base: &str, field: &str) -> (Vec<Self::Stmt>, Self::Expr) {
        (Vec::new(), Self::member(base, field))
    }

    /// The operators, in Elixir's spelling.
    ///
    /// `and`/`or` and not `&&`/`||`: the strict forms take booleans
    /// only, and everything the walk tests is one — a truthiness
    /// operator would quietly accept a `nil` where a boolean belongs.
    /// The bitwise four are `Bitwise`'s operator forms, which the
    /// generated module imports.
    fn binary(op: &'static str, lhs: Self::Expr, rhs: Self::Expr) -> Self::Expr {
        ex::Expr::Binary {
            op: match op {
                "&&" => "and",
                "||" => "or",
                "&" => "&&&",
                "|" => "|||",
                ">>" => ">>>",
                "<<" => "<<<",
                other => other,
            },
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        }
    }

    /// `not a`; bitwise complement is `Bitwise.bnot(a)` because `~~~`
    /// is deprecated in the Elixir this column measures against.
    fn unary(op: &'static str, value: Self::Expr) -> Self::Expr {
        if op == "~" {
            return ex::Expr::Call {
                func: Path::new(&["Bitwise", "bnot"]),
                args: vec![value],
            };
        }
        ex::Expr::Unary {
            op: match op {
                "!" => "not ",
                other => other,
            },
            value: Box::new(value),
        }
    }

    fn paren(value: Self::Expr) -> Self::Expr {
        ex::Expr::Paren(Box::new(value))
    }

    /// The identity on every integer, and a rewrite on three things
    /// that are not integers — the same three as the Python column, at
    /// a union's discriminator.
    fn cast(type_: Self::Type, value: Self::Expr) -> Self::Expr {
        if type_ == Self::named(SIGNED_CHAR) {
            return signed_char(value);
        }
        let ex::Expr::Literal(Literal::Int(text)) = &value else {
            return value;
        };
        let digits = text.strip_prefix('-').unwrap_or(text);
        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return value;
        }
        if type_ == Self::scalar(Scalar::Char) {
            // The one-character binary the discriminator holds. A lone
            // surrogate has no UTF-8 spelling and no Elixir binary can
            // hold one, and the runtime refuses reading one — so a
            // surrogate label becomes an atom no binary ever equals,
            // which is "selects no branch" said in a comparison.
            return text
                .parse::<u32>()
                .ok()
                .filter(|code| *code <= u32::from(u16::MAX))
                .map_or_else(
                    || value.clone(),
                    |code| match char::from_u32(code) {
                        Some(point) => ex::Expr::Literal(Literal::Str(point.to_string())),
                        None => ex::Expr::Atom(format!("zd_surrogate_{code:x}")),
                    },
                );
        }
        if type_ == Self::scalar(Scalar::Bool) {
            return ex::Expr::Literal(Literal::Bool(text != "0"));
        }
        value
    }

    /// The value, unchanged: everything is handed by value and rebuilt
    /// by the caller.
    fn handoff(value: Self::Expr, _form: Handoff) -> Self::Expr {
        value
    }

    // -- statements ----------------------------------------------------

    /// `name = value`, or `name = nil` where the walk declared without
    /// a value — the binding the renderer's threading needs to exist
    /// before the block that rebinds it.
    fn declare(_type: Self::Type, name: &str, value: Option<Self::Expr>) -> Self::Stmt {
        match value {
            Some(value) => Self::seq(Self::assign(Self::name(name), value)),
            None => ex::Statement::Bind {
                name: Ident::new(name),
                value: Self::absent(),
            },
        }
    }

    /// An expression evaluated for its effect — which in this column is
    /// always a rebinding: an assignment the walk built, a runtime call
    /// that threads its receiver, or a generated encoder call that
    /// threads the writer.
    fn perform(value: Self::Expr) -> Self::Stmt {
        if let ex::Expr::Binary { op: "=", lhs, rhs } = value {
            return Self::seq(Self::assign(*lhs, *rhs));
        }
        if let ex::Expr::Call { func, args } = &value {
            let head = func.segments().first().map(Ident::as_str);
            if head == Some(W_MARK) || head == Some(R_MARK) {
                let marker = head.unwrap_or_default().to_string();
                let method = func
                    .segments()
                    .get(1)
                    .map(Ident::as_str)
                    .unwrap_or_default()
                    .to_string();
                let shape = if marker == W_MARK {
                    writer_op(&method)
                } else {
                    reader_op(&method)
                };
                let receiver = Self::receiver(&marker);
                let mut statements = Vec::new();
                let mut temporaries = 0usize;
                let args: Vec<ex::Expr> = args
                    .iter()
                    .cloned()
                    .map(|argument| Self::lower_into(argument, &mut statements, &mut temporaries))
                    .collect();
                let mut full_args = vec![ex::Expr::Var(Ident::new(receiver))];
                full_args.extend(args);
                let call = ex::Expr::Call {
                    func: Self::runtime_path(&marker, &method),
                    args: full_args,
                };
                statements.push(match shape {
                    OpShape::Mutates => ex::Statement::Bind {
                        name: Ident::new(receiver),
                        value: call,
                    },
                    // A value-answering call performed for its effect
                    // still moves the state; the value is discarded by
                    // name.
                    OpShape::MutatesWithValue => ex::Statement::BindTuple {
                        names: vec![Ident::new("_"), Ident::new(receiver)],
                        value: call,
                    },
                    OpShape::Pure => ex::Statement::Bind {
                        name: Ident::new("_"),
                        value: call,
                    },
                });
                return Self::seq(statements);
            }
            if Self::is_encoder_call(func) {
                // `w = zd_enc_T(w, value)`: a generated encoder answers
                // the writer it advanced.
                let (mut statements, lowered) = Self::lower(value.clone());
                statements.push(ex::Statement::Bind {
                    name: Ident::new("w"),
                    value: lowered,
                });
                return Self::seq(statements);
            }
        }
        let (mut statements, lowered) = Self::lower(value);
        statements.push(ex::Statement::Bind {
            name: Ident::new("_"),
            value: lowered,
        });
        Self::seq(statements)
    }

    /// `throw({:zd_return, {v, r}})` — every `give_back` the decode
    /// walk builds is the bare form, and a decoder here answers the
    /// sample *and* the reader it consumed. A `give_back` that ever
    /// arrives with a value answers that value in the sample's place,
    /// which is what the C shape means by it.
    fn give_back(value: Option<Self::Expr>) -> Self::Stmt {
        ex::Statement::Return(ex::Expr::Tuple(vec![
            value.unwrap_or_else(|| Self::name("v")),
            Self::name("r"),
        ]))
    }

    fn when(
        condition: Self::Expr,
        then: Self::Block,
        otherwise: Option<Self::Block>,
    ) -> Self::Stmt {
        // `reader_clear_discard` arrives here as a condition and moves
        // the reader; an `if` evaluates its condition once, so hoisting
        // it out in front is the same program.
        let (mut statements, condition) = Self::lower(condition);
        statements.push(ex::Statement::If {
            condition,
            then,
            otherwise: otherwise.unwrap_or_default(),
        });
        Self::seq(statements)
    }

    fn count_up(counter: &str, bound: Self::Expr, body: Self::Block) -> Self::Stmt {
        let (mut statements, bound) = Self::lower(bound);
        statements.push(ex::Statement::For {
            counter: Ident::new(counter),
            bound,
            body,
        });
        Self::seq(statements)
    }

    /// A loop whose condition is re-evaluated per pass, so a
    /// state-changing call inside it has no lowering at all — the three
    /// conditions the walks build are pure, and a fourth that is not
    /// must stop at the generated file rather than run once: the hoist
    /// is kept *and* a call to a name no module defines goes in front,
    /// so `elixirc` refuses the file and names the fault, and the loop
    /// can never run with a condition evaluated at the wrong time.
    fn until(condition: Self::Expr, body: Self::Block) -> Self::Stmt {
        let (mut statements, condition) = Self::lower(condition);
        if !statements.is_empty() {
            statements.push(ex::Statement::Bind {
                name: Ident::new("_"),
                value: ex::Expr::Call {
                    func: Path::single("zd_refused_stateful_loop_condition"),
                    args: Vec::new(),
                },
            });
        }
        statements.push(ex::Statement::While { condition, body });
        Self::seq(statements)
    }

    fn leave() -> Self::Stmt {
        ex::Statement::Break
    }

    fn note(doc: Doc) -> Self::Stmt {
        ex::Statement::Comment(doc)
    }

    /// A value read for its effect: the reader still moves, the value
    /// is discarded by name. Exactly [`Self::perform`]'s job — a
    /// runtime call performed for its effect binds `{_, receiver}`,
    /// anything else binds `_` — and one implementation rather than a
    /// second copy that could drift into leaving a temporary unread.
    fn drop_value(value: Self::Expr) -> Self::Stmt {
        Self::perform(value)
    }

    /// `_ = zd_tc` — the one spelling Elixir gives "this parameter is
    /// deliberately not read", and warnings are failures in this
    /// column's compile gates.
    fn mark_used(name: &str) -> Vec<Self::Stmt> {
        vec![ex::Statement::Bind {
            name: Ident::new("_"),
            value: Self::name(name),
        }]
    }

    /// `{<place>, r} = zd_dec_T(r, behaviour)` — a generated decoder
    /// answers the value and the reader, and the destination is the
    /// bind's left-hand side.
    fn read_call(
        callee: &str,
        place: Self::Expr,
        _form: Handoff,
        behaviour: Self::Expr,
    ) -> Self::Stmt {
        let call = ex::Expr::Call {
            func: Path::single(callee),
            args: vec![Self::name("r"), behaviour],
        };
        let Some((base, path)) = Self::place_of(&place) else {
            return Self::refused_place(place, call);
        };
        if path.is_empty() {
            return ex::Statement::BindTuple {
                names: vec![base, Ident::new("r")],
                value: call,
            };
        }
        let temporary = Self::fresh("zd_rd");
        Self::seq(vec![
            ex::Statement::BindTuple {
                names: vec![temporary.clone(), Ident::new("r")],
                value: call,
            },
            ex::Statement::Update {
                base,
                path,
                value: ex::Expr::Var(temporary),
            },
        ])
    }

    /// `{v, r} = zd_dec_T(r, zd_tc)` — a `typedef`, handed straight on.
    fn read_forward(callee: &str) -> Self::Stmt {
        ex::Statement::BindTuple {
            names: vec![Ident::new("v"), Ident::new("r")],
            value: ex::Expr::Call {
                func: Path::single(callee),
                args: vec![Self::name("r"), Self::name("zd_tc")],
            },
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
    /// A sequence's slots are assigned outright, so `List.duplicate`
    /// over `nil` is enough; a `map`'s are written half at a time, so
    /// each slot is a two-element list — and unlike the Python column
    /// there is no aliasing to avoid, because an Elixir list is
    /// immutable and `put_in` rebuilds the spine. The entries *stay*
    /// lists; the filler builds lists and the dumper reads them.
    fn make_room(
        base: &str,
        _element: Self::Type,
        count: Self::Expr,
        assigned_whole: bool,
        fill: Vec<Self::Stmt>,
    ) -> Vec<Self::Stmt> {
        let slot = if assigned_whole {
            Self::absent()
        } else {
            ex::Expr::List(vec![Self::absent(), Self::absent()])
        };
        let mut out = vec![ex::Statement::Bind {
            name: Ident::new(base),
            value: ex::Expr::Call {
                func: Path::new(&["List", "duplicate"]),
                args: vec![slot, count],
            },
        }];
        out.extend(fill);
        out
    }

    /// `v = Enum.take(v, count)` — §7.3.1.2.1.12.3's TRIM.
    fn keep_first(base: &str, count: Self::Expr) -> Vec<Self::Stmt> {
        vec![ex::Statement::Bind {
            name: Ident::new(base),
            value: ex::Expr::Call {
                func: Path::new(&["Enum", "take"]),
                args: vec![Self::name(base), count],
            },
        }]
    }

    /// The subscript, with nothing before or after it: a list element
    /// is an ordinary value at every element type.
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

    /// A local the body writes *into*: the `map` scratch entry is a
    /// two-slot list, everything else is an ordinary declaration.
    fn storage(type_: Self::Type, name: &str) -> Self::Stmt {
        if type_ == ex::Type::Atom(MAP_ENTRY.to_string()) {
            return ex::Statement::Bind {
                name: Ident::new(name),
                value: ex::Expr::List(vec![Self::absent(), Self::absent()]),
            };
        }
        Self::declare(type_, name, None)
    }

    // -- assembly ------------------------------------------------------

    /// `def zd_enc_T(w, v) do … w end` and
    /// `def zd_dec_T(r, zd_tc) do v = zd_def_T() … {v, r} end`.
    ///
    /// A forward declaration is answered with nothing: an Elixir module
    /// resolves a local call at run time and a recursive type needs no
    /// declaration ahead of its body.
    fn routine(spec: Routine<Self>) -> Option<Self::Decl> {
        // The routine's body is complete by the time it arrives here, so
        // this is where its temporaries stop being reachable and the
        // counter goes back to zero. Before the early return: a
        // forward declaration carries no body but the walk may well have
        // numbered temporaries deciding that.
        Self::reset_temporaries();
        let body = spec.body?;
        let mut params = Vec::new();
        if spec.writing {
            params.push(Ident::new("w"));
            params.push(Ident::new("v"));
        } else {
            params.push(Ident::new("r"));
        }
        params.extend(spec.extra.iter().map(|(_, name)| Ident::new(name)));
        let (body, result) = if spec.writing {
            // `_ = v` first: an aggregate with no members writes its
            // (absent) delimiter and nothing else, and a parameter the
            // body never reads is a warning — and warnings are failures
            // in this column's compile gates.
            let mut statements = vec![ex::Statement::Bind {
                name: Ident::new("_"),
                value: Self::name("v"),
            }];
            statements.extend(body);
            (statements, Self::name("w"))
        } else {
            let defaulter = crate::default_elixir::defaulter_from_decoder(&spec.name);
            let mut statements = vec![
                ex::Statement::Bind {
                    name: Ident::new("v"),
                    value: ex::Expr::Call {
                        func: Path::single(&defaulter),
                        args: Vec::new(),
                    },
                },
                // `_ = v` reads the Table 9 binding: a scalar's decoder
                // rebinds `v` outright and would otherwise leave the
                // defaulter's binding unread, which is a warning — and
                // warnings are failures in this column's compile gates.
                ex::Statement::Bind {
                    name: Ident::new("_"),
                    value: Self::name("v"),
                },
            ];
            statements.extend(body);
            (
                statements,
                ex::Expr::Tuple(vec![Self::name("v"), Self::name("r")]),
            )
        };
        Some(ex::CodecFunction {
            doc: spec.doc,
            name: Ident::new(&spec.name),
            params,
            body,
            result,
        })
    }

    /// `place = zd_def_T()` — Table 9, through the generated defaulter.
    ///
    /// # Errors
    /// Never; the signature matches the emitters around it.
    fn default_into(
        ast: &Ast,
        id: Id<TypeNode>,
        place: Self::Expr,
    ) -> Result<Self::Stmt, EncodeError> {
        Ok(crate::default_elixir::default_into(ast, id, place))
    }

    /// # Errors
    /// As [`crate::default_elixir::member_default`]: a `@default` this
    /// column cannot spell is refused rather than replaced by Table 9's
    /// zero.
    fn member_default(
        ast: &Ast,
        node: &MemberNode,
        type_: Id<TypeNode>,
        place: Self::Expr,
    ) -> Result<Vec<Self::Stmt>, EncodeError> {
        crate::default_elixir::member_default(ast, node, type_, place)
    }

    /// `Common.Mode.active()` — the enumeration module's constant
    /// function, which answers the declared integer. The materialiser's
    /// own module path, so the two cannot part.
    fn enumerator_constant(ast: &Ast, id: Id<TypeNode>, enumerator: &str) -> Self::Expr {
        let module = type_module_path(ast, id);
        let mut segments: Vec<String> = module
            .segments()
            .iter()
            .map(|segment| segment.as_str().to_string())
            .collect();
        segments.push(atom_name(enumerator));
        ex::Expr::Call {
            func: Path::new(&segments.iter().map(String::as_str).collect::<Vec<&str>>()),
            args: Vec::new(),
        }
    }
}

/// A character's code point read as C's signed `char`:
/// `rem(hd(String.to_charlist(k)) + 128, 256) - 128`.
///
/// `rem` and not `Integer.mod`: the operand is a code point plus 128 and
/// never negative, so the two agree, and `rem` needs no alias. Inline
/// arithmetic and not a runtime helper, for the reason the Python column
/// records: how wide and how signed C spells a character is a language
/// mapping question and does not belong in a runtime whose subject is
/// §7.4.
fn signed_char(value: ex::Expr) -> ex::Expr {
    let point = ex::Expr::Call {
        func: Path::single("hd"),
        args: vec![ex::Expr::Call {
            func: Path::new(&["String", "to_charlist"]),
            args: vec![value],
        }],
    };
    let wrapped = ex::Expr::Call {
        func: Path::single("rem"),
        args: vec![
            ex::Expr::Binary {
                op: "+",
                lhs: Box::new(point),
                rhs: Box::new(ex::Expr::int(128)),
            },
            ex::Expr::int(256),
        ],
    };
    ex::Expr::Binary {
        op: "-",
        lhs: Box::new(ex::Expr::Paren(Box::new(wrapped))),
        rhs: Box::new(ex::Expr::int(128)),
    }
}

/// `strcmp(a, b)` as its sign, computed with Elixir's own binary
/// comparison — bytewise, which is `strcmp`'s `unsigned char` order.
fn strcmp_sign(args: Vec<ex::Expr>) -> ex::Expr {
    let [left, right] = <[ex::Expr; 2]>::try_from(args).unwrap_or_else(|args| {
        // Unreachable for the one call site, which passes exactly two;
        // a different arity is a walk that changed under this file, and
        // the surviving shape is a call to a name no module defines.
        [
            ex::Expr::Call {
                func: Path::single("strcmp"),
                args,
            },
            ex::Expr::int(0),
        ]
    });
    let side = |op: &'static str| {
        ex::Expr::Paren(Box::new(ex::Expr::IfElse {
            condition: Box::new(ex::Expr::Binary {
                op,
                lhs: Box::new(left.clone()),
                rhs: Box::new(right.clone()),
            }),
            then: Box::new(ex::Expr::int(1)),
            otherwise: Box::new(ex::Expr::int(0)),
        }))
    };
    ex::Expr::Paren(Box::new(ex::Expr::Binary {
        op: "-",
        lhs: Box::new(side(">")),
        rhs: Box::new(side("<")),
    }))
}

/// The bound a `string`, a `sequence` or a `map` carries, as the suffix
/// that keeps two of them apart.
///
/// The tag names the routine a hoisted collection's codec is emitted
/// under, and one body is emitted per name. With the bound left out,
/// `sequence<long>` and `sequence<long, 4>` are one name and whichever
/// was declared first decides: measured on
/// `@final struct S { sequence<long> b; sequence<long, 4> a; };`, the
/// generated Elixir held no `zd_over` check at all, so `a` accepted a
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

/// The tag a codec function is named after.
///
/// The Elixir counterpart of `adapter::python::type_tag`: built from
/// the IDL kind for a primitive — `double` and `long double` share
/// nothing here, but the naming convention is one across columns — and
/// from the module path for a declared type, its segments joined by
/// `_` because a function name cannot carry a dot.
///
/// zerodds-lint: recursion-depth 64 (collection nesting; bounded by
/// `MAX_TYPE_DEPTH`)
pub(crate) fn type_tag(ast: &Ast, id: Id<TypeNode>, depth: usize) -> String {
    if depth > MAX_TYPE_DEPTH {
        return "unknown".to_string();
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
                type_tag(ast, element, depth + 1)
            ),
            None => format!("seq_unknown{}", bound_tag(ast, bound)),
        },
        TypeKind::Array { element, dims } => {
            let mut name = match element.resolved() {
                Some(element) => format!("arr_{}", type_tag(ast, element, depth + 1)),
                None => "arr_unknown".to_string(),
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
                None => "unknown".to_string(),
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
            None => "unknown".to_string(),
        },
        TypeKind::ValueBox { boxed } => match boxed.resolved() {
            Some(target) => format!("box_{}", type_tag(ast, target, depth + 1)),
            None => "box_unknown".to_string(),
        },
        TypeKind::Structure { .. }
        | TypeKind::Union { .. }
        | TypeKind::Enumeration { .. }
        | TypeKind::Bitmask { .. }
        | TypeKind::Bitset { .. }
        | TypeKind::Valuetype { .. }
        | TypeKind::Native => type_module_path(ast, id)
            .segments()
            .iter()
            .map(|segment| segment.as_str())
            .collect::<Vec<&str>>()
            .join("_"),
        TypeKind::Any => "any".to_string(),
        TypeKind::ValueBase => "valuebase".to_string(),
        TypeKind::ObjectRef { .. } => "objectref".to_string(),
        TypeKind::AnnotationType { .. } => "annotation".to_string(),
    }
}

/// The Elixir shape of a value, structurally.
///
/// Barely rendered — a codec body carries no annotations — but the
/// comparisons in [`Elixir::cast`] read it, so a `char` place is marked
/// apart from a `string` one although both hold a binary at run time.
///
/// zerodds-lint: recursion-depth 64 (collection nesting; bounded by
/// `MAX_TYPE_DEPTH`)
fn value_type_of(ast: &Ast, id: Id<TypeNode>, depth: usize) -> ex::Type {
    use zerodds_ast::PrimitiveKind;
    let unknown = || ex::Type::Basic(ex::Basic::Any);
    if depth > MAX_TYPE_DEPTH {
        return unknown();
    }
    let Ok(node) = ast.type_(id) else {
        return unknown();
    };
    let inner = |half: Option<Id<TypeNode>>| {
        half.map_or_else(unknown, |target| value_type_of(ast, target, depth + 1))
    };
    match &node.kind {
        TypeKind::Primitive(kind) => match kind {
            PrimitiveKind::Boolean => Elixir::scalar(Scalar::Bool),
            PrimitiveKind::Char | PrimitiveKind::WChar => Elixir::scalar(Scalar::Char),
            PrimitiveKind::Float32 | PrimitiveKind::Float64 => Elixir::scalar(Scalar::Double),
            PrimitiveKind::Float128 => Elixir::scalar(Scalar::LongDouble),
            _ => Elixir::scalar(Scalar::Int32),
        },
        TypeKind::String { .. } | TypeKind::Fixed { .. } => ex::Type::Basic(ex::Basic::Binary),
        // An enumeration member is a plain integer in this binding —
        // the materialiser says why — and a bitmask and a bitset's
        // fields likewise.
        TypeKind::Enumeration { .. } | TypeKind::Bitmask { .. } => {
            ex::Type::Basic(ex::Basic::Integer)
        }
        TypeKind::Bitset { .. }
        | TypeKind::Structure { .. }
        | TypeKind::Union { .. }
        | TypeKind::Valuetype { .. }
        | TypeKind::Native => ex::Type::Atom(type_tag(ast, id, depth)),
        TypeKind::Sequence { element, .. } => ex::Type::list(inner(element.resolved())),
        TypeKind::Array { element, dims } => {
            let mut out = inner(element.resolved());
            for _ in dims {
                out = ex::Type::list(out);
            }
            out
        }
        TypeKind::Map { key, value, .. } => ex::Type::list(ex::Type::Tuple(vec![
            inner(key.resolved()),
            inner(value.resolved()),
        ])),
        TypeKind::Alias { aliased } => inner(aliased.resolved()),
        TypeKind::ValueBox { boxed } => inner(boxed.resolved()).or_nil(),
        TypeKind::Any
        | TypeKind::ValueBase
        | TypeKind::ObjectRef { .. }
        | TypeKind::AnnotationType { .. } => unknown(),
    }
}
