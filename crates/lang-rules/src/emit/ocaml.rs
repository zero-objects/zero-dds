// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The OCaml backend of the shared codec walk.
//!
//! DDS-XTypes 1.3 §7.4 from the clause, the same as `runtime/c`,
//! `runtime/python`, `runtime/go` and the rest do, and none of them
//! calls another: `runtime/ocaml/zdwire.ml` is a further reading of the
//! specification, and this file only spells the walk's decisions in
//! OCaml.
//!
//! # The five decisions this column turns on
//!
//! **There are no statements, so a body is a `unit` expression.** Every
//! [`Stmt`] this backend builds is rendered by
//! `zerodds_lang_render::ocaml::codec_chain` under `;` sequencing, a
//! local is a `ref` cell, and the two control transfers C spells
//! `return` and `break` are the runtime's `Zd_return` and `Zd_break`
//! exceptions — OCaml has neither keyword.
//!
//! **There are no implicit integer conversions, so kinds are data.**
//! `adapter/ocaml.rs` maps `long` onto `int32`, `unsigned long` and
//! `long long` onto `int64`, everything of at most 16 bits onto `int`,
//! and `unsigned long long` onto decimal text. A `+` is `Int64.add` on
//! one pair and `+` on the next, and only the renderer's typing
//! environment can say which — so every [`CodecType`] carries a
//! [`ValueKind`] and the conversion sites carry [`CodeExpr::Convert`]
//! rather than a spelled cast.
//!
//! **Records are immutable, so a member store is a functional update.**
//! The reader's sample lives in a `ref` cell seeded from the Table 9
//! defaulter, and `v.x = e` becomes `v := { !v with x = e }`. Arrays
//! are the one mutable shape OCaml gives this column, and every
//! collection slot is stored with `<-`.
//!
//! **An enumeration is a variant, so its number lives in a function.**
//! The adapter emits `int32_of_<name>` and `<name>_of_int32` beside
//! every enumeration, and this backend reaches the numbering through
//! them — a cast of a variant to `Int64` is
//! `Int64.of_int32 (int32_of_<name> v)`, resolved by kind.
//!
//! **A union branch is an `option` field.** The value of a selected
//! branch is `Option.get v.f` on the way out and `Some …` on the way
//! in, which is [`CodeExpr::Member`]'s `case` flag; a discriminator
//! that selects no branch leaves every branch `None`, which is the
//! record shape `adapter/ocaml.rs` argues for.

use super::{CodecTarget, Constant, Handle, Handoff, Routine, Scalar};
use crate::adapter::ocaml::{
    ocaml_enumerator_name, ocaml_map_entry_name, ocaml_type_name, ocaml_value_type,
};
use crate::encode::EncodeError;
use zerodds_ast::{Ast, CharWidth, MemberNode, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::{Doc, Literal};
use zerodds_lang_cst::ocaml::{self, CodeExpr, CodecFn, CodecType, Stmt, ValueKind};

/// The OCaml backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OCaml;

/// How deep a chain of typedefs and collections is followed.
const MAX_TYPE_DEPTH: usize = 64;

impl OCaml {
    fn type_(text: &str, kind: ValueKind, default: &str) -> CodecType {
        CodecType {
            text: text.to_string(),
            kind,
            default: default.to_string(),
        }
    }

    fn constant_text(text: &str, kind: ValueKind) -> CodeExpr {
        CodeExpr::RuntimeConst {
            text: text.to_string(),
            kind,
        }
    }

    fn policy(text: &str) -> CodeExpr {
        CodeExpr::TextLit(text.to_string())
    }
}

/// The written OCaml type of the adapter's tree — a local copy of the
/// renderer's `type_text`, because this crate does not depend on
/// `zerodds-lang-render` and the postfix grammar is four arms.
pub fn ocaml_type_text(node: &ocaml::Type) -> String {
    match node {
        ocaml::Type::Primitive(primitive) => primitive.as_str().to_string(),
        ocaml::Type::Named(name) => name.as_str().to_string(),
        ocaml::Type::Array(element) => format!("{} array", ocaml_type_text(element)),
        ocaml::Type::Option(inner) => format!("{} option", ocaml_type_text(inner)),
        ocaml::Type::Tuple(parts) => {
            let rendered: Vec<String> = parts.iter().map(ocaml_type_text).collect();
            format!("({})", rendered.join(" * "))
        }
        ocaml::Type::Qualified { module, name } => {
            let mut out = String::new();
            for segment in module {
                out.push_str(segment.as_str());
                out.push('.');
            }
            out.push_str(name.as_str());
            out
        }
    }
}

/// The [`ValueKind`] a value of `id` has in this column, following
/// typedefs — the memory-shape table of `adapter/ocaml.rs`, reduced to
/// what conversions care about.
///
/// zerodds-lint: recursion-depth 64 (alias chains; bounded by `MAX_TYPE_DEPTH`)
pub fn kind_of(ast: &Ast, id: Id<TypeNode>, depth: usize) -> ValueKind {
    if depth > MAX_TYPE_DEPTH {
        return ValueKind::Aggregate;
    }
    let Ok(node) = ast.type_(id) else {
        return ValueKind::Aggregate;
    };
    match &node.kind {
        TypeKind::Primitive(kind) => match kind {
            PrimitiveKind::Boolean => ValueKind::Bool,
            PrimitiveKind::Octet
            | PrimitiveKind::UInt8
            | PrimitiveKind::Char
            | PrimitiveKind::Int8
            | PrimitiveKind::Int16
            | PrimitiveKind::UInt16
            | PrimitiveKind::WChar => ValueKind::Int,
            PrimitiveKind::Int32 => ValueKind::Int32,
            PrimitiveKind::UInt32 | PrimitiveKind::Int64 => ValueKind::Int64,
            PrimitiveKind::UInt64 | PrimitiveKind::Float128 => ValueKind::Text,
            PrimitiveKind::Float32 | PrimitiveKind::Float64 => ValueKind::Float,
        },
        TypeKind::String { .. } | TypeKind::Fixed { .. } => ValueKind::Text,
        TypeKind::Enumeration { .. } => ValueKind::Enum(ocaml_type_name(ast, id)),
        TypeKind::Bitmask { .. } => {
            if ast.effective_bit_bound(id) <= 32 {
                ValueKind::Int32
            } else {
                ValueKind::Int64
            }
        }
        TypeKind::Alias { aliased } => aliased.resolved().map_or(ValueKind::Aggregate, |target| {
            kind_of(ast, target, depth + 1)
        }),
        // The structural kinds carry their parts, so the renderer can
        // type a member access without the model in hand. Capped low —
        // a codec body reaches at most a few accessors deep from `v`,
        // and a recursive type's kind tree would otherwise grow with
        // the alias bound rather than with what any body reads.
        _ if depth > 8 => ValueKind::Aggregate,
        TypeKind::Sequence { element, .. } => {
            element.resolved().map_or(ValueKind::Aggregate, |target| {
                ValueKind::ArrayOf(Box::new(kind_of(ast, target, depth + 1)))
            })
        }
        TypeKind::Array { element, dims } => match element.resolved() {
            Some(target) => {
                let mut kind = kind_of(ast, target, depth + 1);
                for _ in dims {
                    kind = ValueKind::ArrayOf(Box::new(kind));
                }
                kind
            }
            None => ValueKind::Aggregate,
        },
        TypeKind::Map { key, value, .. } => {
            let half = |half: Option<Id<TypeNode>>| {
                half.map_or(ValueKind::Aggregate, |target| {
                    kind_of(ast, target, depth + 1)
                })
            };
            ValueKind::ArrayOf(Box::new(ValueKind::RecordOf(vec![
                ("key".to_string(), half(key.resolved())),
                ("value".to_string(), half(value.resolved())),
            ])))
        }
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let mut fields = Vec::new();
            // The base is the first member `parent` (XTypes 1.3 §7.5.1.4.1.3).
            if let Some(base) = crate::adapter::c::struct_base(ast, id) {
                fields.push((ocaml::lower_ident("parent"), kind_of(ast, base, depth + 1)));
            }
            for member in ast.members_of(id).iter().copied() {
                let Ok(member_node) = ast.member(member) else {
                    continue;
                };
                let Some(target) = member_node.type_.resolved() else {
                    continue;
                };
                let mut kind = kind_of(ast, target, depth + 1);
                if member_node.facets.is_optional() {
                    kind = ValueKind::OptionOf(Box::new(kind));
                }
                fields.push((ocaml::lower_ident(&member_node.name), kind));
            }
            ValueKind::RecordOf(fields)
        }
        TypeKind::Union { discriminator, .. } => {
            let mut fields = Vec::new();
            if let Some(disc) = discriminator.resolved() {
                fields.push(("_d".to_string(), kind_of(ast, disc, depth + 1)));
            }
            for case in ast.cases_of(id) {
                let Ok(case_node) = ast.case(*case) else {
                    continue;
                };
                let Ok(member) = ast.member(case_node.member) else {
                    continue;
                };
                let Some(target) = member.type_.resolved() else {
                    continue;
                };
                fields.push((
                    ocaml::lower_ident(&member.name),
                    ValueKind::OptionOf(Box::new(kind_of(ast, target, depth + 1))),
                ));
            }
            ValueKind::RecordOf(fields)
        }
        TypeKind::Bitset { .. } => {
            let mut fields = Vec::new();
            for field in &ast.all_bitfields_of(id) {
                let Ok(field_node) = ast.bitfield(*field) else {
                    continue;
                };
                let Some(field_name) = field_node.name.as_ref() else {
                    continue;
                };
                let kind = match crate::adapter::ocaml::ocaml_holder(field_node.bits) {
                    ocaml::Primitive::Int32 => ValueKind::Int32,
                    ocaml::Primitive::Int64 => ValueKind::Int64,
                    _ => ValueKind::Int,
                };
                fields.push((ocaml::lower_ident(field_name), kind));
            }
            ValueKind::RecordOf(fields)
        }
        _ => ValueKind::Aggregate,
    }
}

/// The placeholder expression a value of `id` starts as — a literal
/// zero for the scalar kinds, and the generated Table 9 defaulter for
/// everything with structure. What a `ref` cell or an `Array.make`
/// slot holds until the decoder overwrites it; never observed by a
/// completed read.
pub fn default_of(ast: &Ast, id: Id<TypeNode>) -> String {
    match kind_of(ast, id, 0) {
        ValueKind::Bool => "false".to_string(),
        ValueKind::Int => "0".to_string(),
        ValueKind::Int32 => "0l".to_string(),
        ValueKind::Int64 => "0L".to_string(),
        ValueKind::Float => "0.0".to_string(),
        ValueKind::Text => "\"\"".to_string(),
        _ => format!("{} ()", crate::default_ocaml::defaulter_name(ast, id)),
    }
}

impl CodecTarget for OCaml {
    type Expr = CodeExpr;
    type Stmt = Stmt;
    type Block = Vec<Stmt>;
    type Type = CodecType;
    type Decl = CodecFn;

    // -- types --------------------------------------------------------

    /// The OCaml type each scalar local takes. `Index` and `Size` are
    /// `int` — a counter and a stream position — and so is `UInt32`,
    /// because every length local the walks declare with it holds a
    /// value an OCaml `int` carries exactly on the 64-bit targets this
    /// column requires. The 64-bit widths are `int64`, matching the
    /// member table; the decimal holders are `string`.
    fn scalar(kind: Scalar) -> Self::Type {
        match kind {
            Scalar::Bool => Self::type_("bool", ValueKind::Bool, "false"),
            Scalar::Char
            | Scalar::Int8
            | Scalar::UInt8
            | Scalar::Int16
            | Scalar::UInt16
            | Scalar::UInt32
            | Scalar::Index
            | Scalar::Size => Self::type_("int", ValueKind::Int, "0"),
            Scalar::Int32 => Self::type_("int32", ValueKind::Int32, "0l"),
            Scalar::Int64 => Self::type_("int64", ValueKind::Int64, "0L"),
            // The same OCaml type, under the runtime's alias for it, so
            // that a widening the walk asks for as unsigned keeps the
            // bit pattern where one asked for as signed keeps the sign.
            // `zerodds_lang_cst::ocaml::UNSIGNED_HOLDER` carries why.
            Scalar::UInt64 => Self::type_(ocaml::UNSIGNED_HOLDER, ValueKind::Int64, "0L"),
            Scalar::Float | Scalar::Double => Self::type_("float", ValueKind::Float, "0.0"),
            Scalar::LongDouble => Self::type_("string", ValueKind::Text, "\"0.0\""),
        }
    }

    /// A named type; only the `signed char` marker ever reaches a
    /// rendered conversion, and a default that is asked for renders as
    /// an unbound name so `ocamlc` reports the site.
    fn named(name: &str) -> Self::Type {
        Self::type_(name, ValueKind::Aggregate, "zd_unsupported_default")
    }

    fn nothing() -> Self::Type {
        Self::type_("unit", ValueKind::UnitValue, "()")
    }

    /// The runtime's handles, with the inert value each `ref` cell
    /// starts from.
    fn handle(kind: Handle) -> Self::Type {
        match kind {
            Handle::DHeader => Self::type_(
                "Zdwire.dheader",
                ValueKind::Handle("dheader"),
                "Zdwire.no_dheader ()",
            ),
            Handle::EmHeader => Self::type_(
                "Zdwire.emheader",
                ValueKind::Handle("emheader"),
                "Zdwire.emheader_none",
            ),
            Handle::Pid => Self::type_("Zdwire.pid", ValueKind::Handle("pid"), "Zdwire.pid_none"),
            Handle::TryConstruct => Self::type_(
                "Zdwire.try_construct",
                ValueKind::Handle("try_construct"),
                "Zdwire.Discard",
            ),
            Handle::ReaderMark => Self::type_(
                "Zdwire.reader_mark",
                ValueKind::Handle("reader_mark"),
                "Zdwire.mark_none",
            ),
            Handle::PidRead => Self::type_(
                "Zdwire.pid_read",
                ValueKind::Handle("pid_read"),
                "Zdwire.pid_read_none",
            ),
            Handle::EmHeaderRead => Self::type_(
                "Zdwire.emheader_read",
                ValueKind::Handle("emheader_read"),
                "Zdwire.emheader_read_none",
            ),
        }
    }

    /// The member already holds the code point as an `int`, so there is
    /// nothing to convert.
    fn code_point(value: Self::Expr, _narrow: bool) -> Self::Expr {
        value
    }

    /// The runtime's policy values. Strings and integers where
    /// `runtime/ocaml` takes them so — the spellings are
    /// `WstringLengthPolicy::as_str` and its siblings in
    /// `crates/ast-layout/src/key.rs` — and variant constructors for
    /// the enumerated kinds. One arm per constant and no catch-all.
    fn constant(which: Constant) -> Self::Expr {
        match which {
            Constant::WcharWidth2 => CodeExpr::Int(2),
            Constant::WcharWidth4 => CodeExpr::Int(4),
            Constant::WcharFixed => Self::policy("fixed"),
            Constant::WcharGiopLengthPrefixed => Self::policy("giop-length-prefixed"),
            Constant::WstringUnitsStream => Self::policy("stream"),
            Constant::WstringUnitsBigEndianAlways => Self::policy("big-endian-always"),
            Constant::WstringUnitsByteOrderMark => Self::policy("byte-order-mark"),
            Constant::WstringUnitsHostUnswapped => Self::policy("host-unswapped"),
            Constant::WstringSpecification => Self::policy("specification"),
            Constant::WstringCharacterCount => Self::policy("character-count"),
            Constant::WstringCharacterCountTerminated => Self::policy("character-count-terminated"),
            Constant::WstringSpecificationWideUnits => Self::policy("specification-wide-units"),
            Constant::WstringRosTypeSupport => Self::policy("ros-type-support"),
            Constant::WstringOctetsTerminated2 => Self::policy("octets/terminated/2"),
            Constant::WstringOctetsTerminated4 => Self::policy("octets/terminated/4"),
            Constant::WstringCharactersTerminated2 => Self::policy("characters/terminated/2"),
            Constant::FixedDeclared => Self::policy("declared"),
            Constant::FixedValuePrecision => Self::policy("value-precision"),
            Constant::FixedValueStringDigits => Self::policy("value-string-digits"),
            Constant::ParameterMember => {
                Self::constant_text("Zdwire.Role_member", ValueKind::Handle("role"))
            }
            Constant::ParameterOptional => {
                Self::constant_text("Zdwire.Role_optional", ValueKind::Handle("role"))
            }
            Constant::TryConstructDiscard => {
                Self::constant_text("Zdwire.Discard", ValueKind::Handle("try_construct"))
            }
            Constant::TryConstructTrim => {
                Self::constant_text("Zdwire.Trim", ValueKind::Handle("try_construct"))
            }
            Constant::TryConstructUseDefault => {
                Self::constant_text("Zdwire.Use_default", ValueKind::Handle("try_construct"))
            }
            Constant::StatusOk => Self::constant_text("Zdwire.St_ok", ValueKind::Handle("status")),
            Constant::StatusDiscarded => {
                Self::constant_text("Zdwire.St_discarded", ValueKind::Handle("status"))
            }
            Constant::StatusImpossibleLength => {
                Self::constant_text("Zdwire.St_impossible_length", ValueKind::Handle("status"))
            }
            Constant::StatusOutOfRange => {
                Self::constant_text("Zdwire.St_out_of_range", ValueKind::Handle("status"))
            }
            Constant::StatusTruncated => {
                Self::constant_text("Zdwire.St_truncated", ValueKind::Handle("status"))
            }
        }
    }

    /// Whether `left` is less than `right` as unsigned 64-bit values.
    /// The keys arrive as their decimal holders, and the runtime's
    /// `u64_less` spells `Int64.unsigned_compare` over their patterns.
    fn unsigned_less(_octets: u8, left: Self::Expr, right: Self::Expr) -> Self::Expr {
        CodeExpr::Static {
            method: "u64_less".to_string(),
            args: vec![left, right],
        }
    }

    // `bool_less` keeps the default `<`: OCaml's polymorphic order has
    // `false < true`, measured in the toplevel before this column was
    // written.

    /// `runtime/ocaml` carries the three HDDS axes: `begin_packed_element`
    /// / `finish_packed_element` on the writer and
    /// `reader_begin_packed_element` / `reader_finish_packed_element` on the
    /// reader, the DHEADER pair for a sequence element's own length, and
    /// `write_wstring_utf8_octets` / `read_wstring_utf8_octets`.
    fn carries_hdds_axes() -> bool {
        true
    }

    /// The holder a bit set is assembled in: the adapter's own answer.
    fn unsigned_holder(bits: u16) -> Self::Type {
        match crate::adapter::ocaml::ocaml_holder(bits) {
            ocaml::Primitive::Int32 => Self::type_("int32", ValueKind::Int32, "0l"),
            ocaml::Primitive::Int64 => Self::type_("int64", ValueKind::Int64, "0L"),
            _ => Self::type_("int", ValueKind::Int, "0"),
        }
    }

    fn value_type(ast: &Ast, id: Id<TypeNode>) -> Self::Type {
        let text = match ocaml_value_type(ast, id) {
            Ok(type_) => ocaml_type_text(&type_),
            // A type with no OCaml form was refused by the materialiser
            // before a codec walk ran; the unbound name makes `ocamlc`
            // fail loudly rather than compile a plausible member.
            Err(_) => "zd_no_ocaml_form".to_string(),
        };
        CodecType {
            text,
            kind: kind_of(ast, id, 0),
            default: default_of(ast, id),
        }
    }

    fn type_name(ast: &Ast, id: Id<TypeNode>) -> String {
        type_tag(ast, id, 0)
    }

    fn map_entry_name(ast: &Ast, id: Id<TypeNode>) -> String {
        ocaml_map_entry_name(ast, id)
    }

    /// The entry record's type, with an inline default the reader's
    /// `Array.make` can replicate: the slots are replaced by functional
    /// update, so one shared record value is sound.
    fn map_entry_type(ast: &Ast, id: Id<TypeNode>) -> Self::Type {
        let name = ocaml_map_entry_name(ast, id);
        let default = match ast.type_(id).map(|node| node.kind.clone()) {
            Ok(TypeKind::Map { key, value, .. }) => match (key.resolved(), value.resolved()) {
                (Some(key), Some(value)) => format!(
                    "({{ key = {}; value = {} }} : {name})",
                    default_of(ast, key),
                    default_of(ast, value)
                ),
                _ => "zd_unsupported_default".to_string(),
            },
            _ => "zd_unsupported_default".to_string(),
        };
        CodecType {
            text: name,
            kind: ValueKind::Aggregate,
            default,
        }
    }

    /// Nothing: [`Self::enumerator_constant`] is overridden and no
    /// other seam reads a prefix in this column.
    fn module_prefix(_ast: &Ast, _id: Id<TypeNode>) -> String {
        String::new()
    }

    fn escape(name: &str) -> String {
        ocaml::lower_ident(name)
    }

    /// The base is the first member `parent` (XTypes 1.3 §7.5.1.4.1.3).
    fn inherited_path(depth: usize) -> Option<String> {
        Some(format!("{}.", ocaml::lower_ident("parent")).repeat(depth))
    }

    // -- expressions ---------------------------------------------------

    fn name(text: &str) -> Self::Expr {
        CodeExpr::Name(text.to_string())
    }

    fn int(value: i128) -> Self::Expr {
        CodeExpr::Int(value)
    }

    /// A widthless literal; the renderer spells it in the width of
    /// whatever it meets, which is what OCaml's suffixed literals need.
    fn wide_int(value: i128) -> Self::Expr {
        CodeExpr::Int(value)
    }

    /// The number with no suffix: `5u` is not an OCaml token.
    fn unsigned_int(value: i128) -> Self::Expr {
        CodeExpr::Int(value)
    }

    /// The 64-bit pattern as an `int64` literal — the sign is the top
    /// bit's, because OCaml has no unsigned integer to spell it in.
    #[allow(clippy::cast_possible_wrap)]
    fn wide_unsigned(value: u64) -> Self::Expr {
        CodeExpr::Int64Lit(value as i64)
    }

    #[allow(clippy::cast_possible_truncation)]
    fn wide_signed(value: i128) -> Self::Expr {
        CodeExpr::Int64Lit(value as i64)
    }

    /// The two raw tokens the walk escapes through [`Literal::Int`],
    /// rewritten into what OCaml spells them as.
    fn literal(value: Literal) -> Self::Expr {
        match value {
            Literal::Int(text) => match text.as_str() {
                "true" => CodeExpr::BoolLit(true),
                "false" => CodeExpr::BoolLit(false),
                other => other
                    .parse::<i128>()
                    .map_or_else(|_| CodeExpr::TextLit(other.to_string()), CodeExpr::Int),
            },
            Literal::Bool(flag) => CodeExpr::BoolLit(flag),
            Literal::Str(text) => CodeExpr::TextLit(text),
            Literal::Float(text) => CodeExpr::FloatLit(text),
            Literal::Char(ch) => CodeExpr::Int(i128::from(u32::from(ch))),
            Literal::Bytes(_) | Literal::Null => CodeExpr::Absent,
        }
    }

    /// A call to a generated function; `strcmp` is the one name the
    /// walk hands here that is the runtime's.
    fn call(callee: &str, args: Vec<Self::Expr>) -> Self::Expr {
        if callee == "strcmp" {
            return CodeExpr::Static {
                method: "strcmp".to_string(),
                args,
            };
        }
        CodeExpr::Call {
            callee: callee.to_string(),
            args,
        }
    }

    /// `Zdwire.<method> <receiver> <args>` — the receiver-first shape a
    /// language without methods gives an operation on the writer or the
    /// reader.
    fn runtime(receiver: &str, method: &str, args: Vec<Self::Expr>) -> Self::Expr {
        // The walk's bare `read_pid()` reads a header *and* opens its
        // value block — `read_pid_as` in this runtime, the same rewrite
        // the Python and Go backends make and for the same §7.4.1.2
        // reason.
        if method == "read_pid" && args.is_empty() {
            return Self::runtime(
                receiver,
                "read_pid_as",
                vec![Self::constant(Constant::ParameterMember)],
            );
        }
        let receiver = if receiver == "r" { "r" } else { "w" };
        CodeExpr::Runtime {
            receiver,
            method: method.to_string(),
            args,
        }
    }

    /// A free function of the runtime.
    fn runtime_static(method: &str, args: Vec<Self::Expr>) -> Self::Expr {
        CodeExpr::Static {
            method: method.to_string(),
            args,
        }
    }

    fn member(base: &str, field: &str) -> Self::Expr {
        CodeExpr::Member {
            base: base.to_string(),
            field: field.to_string(),
            case: false,
        }
    }

    fn whole(base: &str) -> Self::Expr {
        CodeExpr::Whole(base.to_string())
    }

    fn field_of(value: Self::Expr, field: &str) -> Self::Expr {
        CodeExpr::HeaderField {
            value: Box::new(value),
            field: field.to_string(),
        }
    }

    fn index(value: Self::Expr, at: Self::Expr) -> Self::Expr {
        CodeExpr::Index {
            base: Box::new(value),
            index: Box::new(at),
        }
    }

    fn count_of(base: &str, fixed: Option<u64>) -> Self::Expr {
        match fixed {
            Some(bound) => CodeExpr::Int(i128::from(bound)),
            None => CodeExpr::CountOf(base.to_string()),
        }
    }

    fn elements(base: &str) -> Self::Expr {
        CodeExpr::Elements(base.to_string())
    }

    /// A union branch, whose field is an `option`: `Option.get` as a
    /// value, `Some` as a destination.
    fn case_member(base: &str, field: &str) -> Self::Expr {
        CodeExpr::Member {
            base: base.to_string(),
            field: field.to_string(),
            case: true,
        }
    }

    fn entry_half(entry: Self::Expr, key: bool) -> Self::Expr {
        CodeExpr::EntryHalf {
            entry: Box::new(entry),
            key,
        }
    }

    fn absent() -> Self::Expr {
        CodeExpr::Absent
    }

    /// Nothing: an OCaml string owns its storage and a read that could
    /// not allocate has already raised.
    fn materialised(_value: Self::Expr) -> Option<Self::Expr> {
        None
    }

    /// `Option.get v.f` — only ever reached inside a branch the
    /// presence test guarded.
    fn optional_value(_ast: &Ast, _id: Id<TypeNode>, base: &str, field: &str) -> Self::Expr {
        CodeExpr::OptionGet(Box::new(Self::member(base, field)))
    }

    /// `v.f <> None`.
    fn present(base: &str, field: &str) -> Self::Expr {
        CodeExpr::IsSome {
            base: base.to_string(),
            field: field.to_string(),
        }
    }

    /// Nothing on the way in — the `Some` store is what makes the
    /// member present — and `None` on the way out.
    fn set_present(base: &str, field: &str, present: bool) -> Vec<Self::Stmt> {
        if present {
            return Vec::new();
        }
        vec![Stmt::Assign {
            place: Self::member(base, field),
            value: CodeExpr::Absent,
        }]
    }

    /// `None` in every case: a branch field is an `option`, `None` unless
    /// the discriminator selects it. The same statement as an absent
    /// `@optional` member, [`Self::set_present`] with `false`.
    fn clear_union_cases(base: &str, fields: &[String]) -> Vec<Self::Stmt> {
        fields
            .iter()
            .flat_map(|field| Self::set_present(base, field, false))
            .collect()
    }

    /// The marked slot; the store wraps the decoded value in `Some`.
    fn optional_destination(base: &str, field: &str) -> (Vec<Self::Stmt>, Self::Expr) {
        (
            Vec::new(),
            CodeExpr::OptionSlot {
                base: base.to_string(),
                field: field.to_string(),
            },
        )
    }

    fn binary(op: &'static str, lhs: Self::Expr, rhs: Self::Expr) -> Self::Expr {
        CodeExpr::Binary {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        }
    }

    fn unary(op: &'static str, value: Self::Expr) -> Self::Expr {
        CodeExpr::Unary {
            op,
            value: Box::new(value),
        }
    }

    fn paren(value: Self::Expr) -> Self::Expr {
        CodeExpr::Paren(Box::new(value))
    }

    /// A conversion, deferred: the renderer resolves it from the
    /// operand's kind, because OCaml's conversions are functions and
    /// which function depends on both ends.
    fn cast(type_: Self::Type, value: Self::Expr) -> Self::Expr {
        CodeExpr::Convert {
            target: type_,
            value: Box::new(value),
        }
    }

    /// The value, unchanged: OCaml passes every value as itself.
    fn handoff(value: Self::Expr, _form: Handoff) -> Self::Expr {
        value
    }

    // -- statements ----------------------------------------------------

    /// A `ref` cell: OCaml's one shape for a local the body assigns.
    fn declare(type_: Self::Type, name: &str, value: Option<Self::Expr>) -> Self::Stmt {
        Stmt::Let {
            type_,
            name: name.to_string(),
            value,
        }
    }

    /// An expression evaluated for its effect — or, where the walk
    /// built an assignment, the assignment: this is the only place the
    /// walk's `=` arrives.
    fn perform(value: Self::Expr) -> Self::Stmt {
        if let CodeExpr::Binary { op: "=", lhs, rhs } = value {
            return Stmt::Assign {
                place: *lhs,
                value: *rhs,
            };
        }
        Stmt::Perform(value)
    }

    /// `raise Zdwire.Zd_return`: OCaml has no `return`, and the codec
    /// function's own `try` is the landing site. The reader's `!v`
    /// after it is what goes back.
    fn give_back(_value: Option<Self::Expr>) -> Self::Stmt {
        Stmt::Return
    }

    fn when(
        condition: Self::Expr,
        then: Self::Block,
        otherwise: Option<Self::Block>,
    ) -> Self::Stmt {
        Stmt::When {
            condition,
            then,
            otherwise: otherwise.filter(|body| !body.is_empty()),
        }
    }

    fn count_up(counter: &str, bound: Self::Expr, body: Self::Block) -> Self::Stmt {
        Stmt::For {
            counter: counter.to_string(),
            bound,
            body,
        }
    }

    fn until(condition: Self::Expr, body: Self::Block) -> Self::Stmt {
        Stmt::While { condition, body }
    }

    /// `raise Zdwire.Zd_break`: OCaml has no `break`, and every
    /// generated loop is rendered inside its own handler.
    fn leave() -> Self::Stmt {
        Stmt::Leave
    }

    fn note(doc: Doc) -> Self::Stmt {
        Stmt::Note(doc)
    }

    /// `ignore e` — a value read for its effect says so.
    fn drop_value(value: Self::Expr) -> Self::Stmt {
        Stmt::Discard(value)
    }

    /// `ignore <name>`: quiet under every warning set, and honest about
    /// a parameter or a latch nothing else reads.
    fn mark_used(name: &str) -> Vec<Self::Stmt> {
        vec![Stmt::Discard(CodeExpr::Name(name.to_string()))]
    }

    /// `place <- zd_dec_T r behaviour` — this column's decoders answer
    /// with the value, and the destination decides its own store shape.
    fn read_call(
        callee: &str,
        place: Self::Expr,
        _form: Handoff,
        behaviour: Self::Expr,
    ) -> Self::Stmt {
        Stmt::ReadInto {
            callee: callee.to_string(),
            place,
            behaviour,
        }
    }

    /// `v := zd_dec_T r zd_tc` — a typedef, handed straight on.
    fn read_forward(callee: &str) -> Self::Stmt {
        Stmt::ReadInto {
            callee: callee.to_string(),
            place: CodeExpr::Whole("v".to_string()),
            behaviour: CodeExpr::Name("zd_tc".to_string()),
        }
    }

    fn block(statements: Vec<Self::Stmt>) -> Self::Block {
        statements
    }

    fn into_statements(block: Self::Block) -> Vec<Self::Stmt> {
        block
    }

    /// `base := Array.make n <element default>`, and the fill after it.
    /// The slots hold one shared placeholder until the fill replaces
    /// them — sound for a whole-slot store, and sound for a map's
    /// half-at-a-time store too, because a half store is a functional
    /// update that makes the slot its own record.
    fn make_room(
        base: &str,
        element: Self::Type,
        count: Self::Expr,
        _assigned_whole: bool,
        fill: Vec<Self::Stmt>,
    ) -> Vec<Self::Stmt> {
        let mut out = vec![Stmt::Alloc {
            base: base.to_string(),
            element,
            count,
        }];
        out.extend(fill);
        out
    }

    /// `base := Array.sub !base 0 n` — §7.3.1.2.1.12.3's TRIM.
    fn keep_first(base: &str, count: Self::Expr) -> Vec<Self::Stmt> {
        vec![Stmt::KeepFirst {
            base: base.to_string(),
            count,
        }]
    }

    /// The subscript, with nothing around it: an OCaml array element is
    /// a real place at every element type.
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

    /// The map's scratch entry is not declared at all: the counting
    /// pass only consumes the stream, so every store into it renders as
    /// `ignore`. Any other storage request is an ordinary cell.
    fn storage(type_: Self::Type, name: &str) -> Self::Stmt {
        if type_.text.starts_with("zd_map_entry_") {
            return Stmt::Scratch {
                name: name.to_string(),
            };
        }
        Stmt::Let {
            type_,
            name: name.to_string(),
            value: None,
        }
    }

    // -- assembly ------------------------------------------------------

    /// One function of the generated `let rec … and …` chain. A forward
    /// declaration is answered with nothing: the chain resolves every
    /// name in it, whatever order the walk visited the types in.
    fn routine(spec: Routine<Self>) -> Option<Self::Decl> {
        let body = spec.body?;
        let defaulter = if spec.writing {
            None
        } else {
            Some(crate::default_ocaml::defaulter_from_decoder(&spec.name))
        };
        Some(CodecFn {
            doc: spec.doc,
            name: spec.name,
            sample: spec.sample,
            reading: !spec.writing,
            defaulter,
            extra: spec.extra,
            body,
        })
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
        Ok(Stmt::Assign {
            place,
            value: CodeExpr::Call {
                callee: crate::default_ocaml::defaulter_name(ast, id),
                args: Vec::new(),
            },
        })
    }

    /// # Errors
    /// As [`crate::default_ocaml::member_default`]: a `@default` this
    /// column cannot spell is refused rather than silently replaced by
    /// Table 9's zero.
    fn member_default(
        ast: &Ast,
        node: &MemberNode,
        type_: Id<TypeNode>,
        place: Self::Expr,
    ) -> Result<Vec<Self::Stmt>, EncodeError> {
        crate::default_ocaml::member_default(ast, node, type_, place)
    }

    /// The constructor, in the adapter's spelling — the one place the
    /// numbering's names live.
    fn enumerator_constant(ast: &Ast, id: Id<TypeNode>, enumerator: &str) -> Self::Expr {
        CodeExpr::Construct(ocaml_enumerator_name(ast, id, enumerator))
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
/// generated OCaml held no `zd_over` check at all, so `a` accepted a
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

/// The name a codec function takes for the type it codes.
///
/// The IDL kind and not the OCaml spelling, for the reason the Python
/// column's `type_tag` gives at length: `octet` and `wchar` are one
/// OCaml type here and two wire forms, and a tag built from the OCaml
/// type would give them one codec. A declared type takes its flattened
/// OCaml name, which two IDL declarations cannot share —
/// `adapter/ocaml.rs`'s `remember` refuses the collision first.
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
        | TypeKind::Native => ocaml_type_name(ast, id),
        TypeKind::Any => "any".to_string(),
        TypeKind::ValueBase => "valuebase".to_string(),
        TypeKind::ObjectRef { .. } => "objectref".to_string(),
        TypeKind::AnnotationType { .. } => "annotation".to_string(),
    }
}
