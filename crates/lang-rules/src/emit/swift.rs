// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Swift backend of the shared codec walk.
//!
//! DDS-XTypes 1.3 §7.4 from the clause, the same as the columns before
//! it, and none of them calls another. Where two columns disagree on an
//! octet, one has misread the specification — which is the only reason
//! to have another.
//!
//! # The five decisions this column turns on
//!
//! **Every conversion is a call.** Swift has no implicit numeric
//! conversion and no cast operator that reaches an enumeration, so the
//! walk's casts all go through `runtime/swift`'s `zdTo(T.self, v)` —
//! whose overloads take a `BinaryInteger` or a `RawRepresentable` on
//! either side. That is what lets one seam cover an `Int64` holder fed
//! from an `enum Mode: Int32`, an `OptionSet` bitmask handed to
//! `writeBitmask`, and a union label compared against an enumerated
//! discriminator.
//!
//! **The loop counter is the bound's own type.** [`Scalar::Index`] is
//! `UInt32` — a sequence count is a `uint32` on the wire — and
//! [`CodecTarget::count_up`] renders `for i in 0..<bound`, which types
//! the counter from the bound. Swift has no mixed-width integer `<`, so
//! homogeneity here is what keeps every comparison in a body compiling.
//! A subscript takes `Int` and nothing else, which is why
//! [`CodecTarget::index`] wraps every index in `Int(..)`.
//!
//! **A union branch is an `Optional` and the seams keep the two sides
//! of it apart.** `adapter/swift.rs` declares one `T?` per branch. As a
//! *value* the branch reads force-unwrapped — `v.x!`, inside the arm
//! the discriminator selected — and as a *place* it is the bare
//! property, because `v.x! = ..` traps on the `nil` every decode starts
//! from while `v.x = ..` promotes. [`SwiftExpr`] carries which of the
//! two a site meant.
//!
//! **A sequence is read by appending.** Swift has no uninitialised
//! array slot, so [`CodecTarget::make_room`] answers `v = []` plus a
//! `reserveCapacity`, and [`CodecTarget::sequence_element`] marks the
//! element's place so the read renders `v.append(..)`. A map's entries
//! are written *into* — both halves per slot — so its room is
//! `[Entry](repeating: zd_def_Entry(), count: n)`, over the entry
//! defaulter `default_swift` emits for exactly this.
//!
//! **`@optional` is `nil` and needs no wrapper on the way in.** A `T`
//! assigned to a `T?` promotes, so the optional destination is the
//! member itself; only `@external` routes through a constructor, the
//! `ZdExternal(..)` box the adapter declares.

use super::{CodecTarget, Constant, Handle, Handoff, Routine, Scalar};
use crate::adapter::swift::{
    property_name, swift_holder, swift_map_entry_name, swift_type_name, swift_value_type,
};
use crate::encode::EncodeError;
use zerodds_ast::{Ast, CharWidth, MemberNode, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::{Doc, Ident, Literal};
use zerodds_lang_cst::swift;

/// The Swift backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Swift;

/// How deep a chain of typedefs and collections is followed.
const MAX_TYPE_DEPTH: usize = 64;

/// The type name [`CodecTarget::named`] is handed for C's signed
/// `char`; the Swift member is a `UInt8` and the signed reading is
/// `Int8(bitPattern:)`.
const SIGNED_CHAR: &str = "signed char";

/// An expression, with the one property the walk's seams cannot carry:
/// what kind of place the site names.
#[derive(Debug, Clone)]
pub enum SwiftExpr {
    /// An ordinary expression.
    Plain(swift::Expr),
    /// A union branch's `T?` property. As a place it is the property —
    /// assignment promotes — and as a value it is force-unwrapped,
    /// because the branch's codec takes the `T`.
    BranchSlot(swift::Expr),
    /// An `@external` member's `ZdExternal<T>` field, as a place a
    /// decoded `T` is about to be written into: the assignment routes
    /// through the box's constructor.
    ExternalSlot(swift::Expr),
    /// A sequence element while it is being read: the collection in
    /// `0` grows by appending, because Swift has no uninitialised slot
    /// to write into.
    AppendSlot(swift::Expr),
    /// The walk's `=`, kept apart so [`Swift::perform`] can route the
    /// place through the slot markers above.
    Assign(Box<SwiftExpr>, Box<swift::Expr>),
}

/// The expression as a *value*, whichever form it took.
/// zerodds-lint: recursion-depth 2 (an Assign holds one place, never another Assign)
fn inner(expr: SwiftExpr) -> swift::Expr {
    match expr {
        SwiftExpr::Plain(e) | SwiftExpr::ExternalSlot(e) | SwiftExpr::AppendSlot(e) => e,
        SwiftExpr::BranchSlot(e) => swift::Expr::ForceUnwrap(Box::new(e)),
        // Never a value; rendered as the statement it is if it leaks.
        SwiftExpr::Assign(place, value) => swift::Expr::Binary {
            op: "=",
            lhs: Box::new(inner(*place)),
            rhs: value,
        },
    }
}

fn plain(e: swift::Expr) -> SwiftExpr {
    SwiftExpr::Plain(e)
}

impl Swift {
    /// A runtime operation's Swift spelling: `write_u32` is `writeU32`
    /// — the first segment as it is, every later segment with its
    /// first letter raised — the names `runtime/swift` publishes,
    /// because that runtime was written against this conversion.
    fn camel(method: &str) -> String {
        let mut out = String::new();
        for (index, segment) in method.split('_').enumerate() {
            let mut chars = segment.chars();
            match chars.next() {
                Some(first) if index > 0 => {
                    out.extend(first.to_uppercase());
                    out.push_str(chars.as_str());
                }
                Some(first) => {
                    out.push(first);
                    out.push_str(chars.as_str());
                }
                None => {}
            }
        }
        out
    }

    /// A free function of the runtime: `no_dheader` is `zdNoDheader`.
    fn free_name(method: &str) -> String {
        let mut out = String::from("zd");
        for segment in method.split('_') {
            let mut chars = segment.chars();
            if let Some(first) = chars.next() {
                out.extend(first.to_uppercase());
                out.push_str(chars.as_str());
            }
        }
        out
    }

    /// Whether `runtime/swift`'s method for `method` (the walk's own
    /// name, before [`Self::camel`]) is `throws`.
    ///
    /// A malformed stream or an out-of-range sample is a `ZdWireError`
    /// every such method throws; a method that only reports the
    /// reader's own bookkeeping — its position, its status, a mark to
    /// restore — never refuses and never throws. This is the one place
    /// that answers the question, so a method the runtime adds without
    /// updating it fails loudly: an untracked throwing call compiles
    /// into a call with no `try`, which `swiftc` refuses outright,
    /// while a method wrongly listed here compiles into a `try` with
    /// nothing to catch, which `swiftc` also refuses — either mistake is
    /// caught at the type-check gate `crates/lang-rules/tests/swift_codec.rs`
    /// runs, not silently.
    fn runtime_method_throws(method: &str) -> bool {
        !matches!(
            method,
            "reader_position"
                | "reader_remaining"
                | "reader_status"
                | "reader_refuse"
                | "reader_clear_discard"
                | "reader_save"
                | "reader_restore"
                | "finish_pid_read"
                | "trim_utf8"
                | "trim_utf16"
                | "position"
                | "finish"
                | "write_bool"
                | "write_u8"
                | "write_i8"
                | "write_char"
                | "write_i16"
                | "write_u16"
                | "write_i32"
                | "write_u32"
                | "write_i64"
                | "write_u64"
                | "write_f32"
                | "write_f64"
                | "write_string"
                | "write_u16_little"
                | "write_u16_big"
                | "write_u32_little"
                | "write_u32_big"
                | "write_dheader"
                | "begin_packed_element"
                | "finish_packed_element"
                | "reader_begin_packed_element"
                | "reader_finish_packed_element"
                | "read_u16_little"
                | "read_u16_big"
                | "read_u32_little"
                | "read_u32_big"
        )
    }

    /// If `expr` is `Expr::Try`, take the throwing call out of it and
    /// report that whatever this becomes part of has to carry the
    /// `try` itself. Swift refuses `try` to the right of a
    /// non-assignment operator, so a combinator that is about to fold
    /// `expr` into a `Binary` or a `Unary` cannot leave it where it is —
    /// it has to hoist the `try` to wrap the combinator's own result
    /// instead, which is what every caller of this does with the
    /// `bool` it gets back.
    fn take_try(expr: swift::Expr) -> (swift::Expr, bool) {
        match expr {
            swift::Expr::Try(inner) => (*inner, true),
            other => (other, false),
        }
    }

    /// `expr`, wrapped in `Try` when `throws` says the operation it
    /// came from does — the one place that decides whether a `try` is
    /// needed at all, so every caller just hands back what this
    /// answers.
    fn maybe_try(expr: swift::Expr, throws: bool) -> SwiftExpr {
        if throws {
            plain(swift::Expr::Try(Box::new(expr)))
        } else {
            plain(expr)
        }
    }

    fn text(value: &str) -> SwiftExpr {
        plain(swift::Expr::Literal(Literal::Str(value.to_string())))
    }

    fn dotted(base: &str, name: &str) -> SwiftExpr {
        plain(swift::Expr::Member {
            value: Box::new(swift::Expr::Name(Ident::new(base))),
            name: Ident::new(name),
        })
    }

    fn invoke(receiver: Option<swift::Expr>, name: &str, args: Vec<swift::Expr>) -> swift::Expr {
        swift::Expr::Invoke {
            receiver: receiver.map(Box::new),
            name: Ident::new(name),
            args: args
                .into_iter()
                .map(|value| swift::Argument { label: None, value })
                .collect(),
        }
    }

    /// `zdTo(<T>.self, value)` — the one conversion seam; the module
    /// documentation says why every cast is a call here.
    fn convert(type_: swift::Type, value: swift::Expr) -> swift::Expr {
        Self::invoke(
            None,
            "zdTo",
            vec![swift::Expr::TypeSelf(Box::new(type_)), value],
        )
    }

    /// One assignment, routed by what the place is.
    fn assemble_assign(place: SwiftExpr, value: swift::Expr) -> swift::Stmt {
        match place {
            SwiftExpr::Plain(p) | SwiftExpr::BranchSlot(p) => {
                swift::Stmt::Assign { place: p, value }
            }
            SwiftExpr::ExternalSlot(p) => swift::Stmt::Assign {
                place: p,
                value: Self::invoke(None, "ZdExternal", vec![value]),
            },
            SwiftExpr::AppendSlot(base) => {
                swift::Stmt::Expr(Self::invoke(Some(base), "append", vec![value]))
            }
            SwiftExpr::Assign(next, first) => {
                // `a = (b = c)` is not Swift and the walk never builds
                // it; keep the inner assignment and drop the outer.
                let _ = first;
                Self::assemble_assign(*next, value)
            }
        }
    }
}

impl CodecTarget for Swift {
    type Expr = SwiftExpr;
    type Stmt = swift::Stmt;
    type Block = Vec<swift::Stmt>;
    type Type = swift::Type;
    type Decl = swift::Decl;

    // -- types --------------------------------------------------------

    /// The fixed-width Swift type each scalar local takes. `Index` is
    /// `UInt32` — a sequence length is a `uint32` on the wire, and the
    /// counter has to be the bound's own type because Swift's `<` takes
    /// no mixed widths — and `Size` is `Int`, which is what the
    /// reader's positions answer in and what a subscript takes.
    fn scalar(kind: Scalar) -> Self::Type {
        match kind {
            Scalar::Bool => swift::Type::Primitive(swift::Primitive::Bool),
            Scalar::Char | Scalar::UInt8 => swift::Type::Primitive(swift::Primitive::UInt8),
            Scalar::Int8 => swift::Type::Primitive(swift::Primitive::Int8),
            Scalar::Int16 => swift::Type::Primitive(swift::Primitive::Int16),
            Scalar::UInt16 => swift::Type::Primitive(swift::Primitive::UInt16),
            Scalar::Int32 => swift::Type::Primitive(swift::Primitive::Int32),
            Scalar::UInt32 | Scalar::Index => swift::Type::Primitive(swift::Primitive::UInt32),
            Scalar::Int64 => swift::Type::Primitive(swift::Primitive::Int64),
            Scalar::UInt64 => swift::Type::Primitive(swift::Primitive::UInt64),
            Scalar::Float => swift::Type::Primitive(swift::Primitive::Float),
            Scalar::Double => swift::Type::Primitive(swift::Primitive::Double),
            // The decimal text; `adapter/swift.rs` says why.
            Scalar::LongDouble => swift::Type::Primitive(swift::Primitive::String),
            // `Int` is deliberately not in the CST's primitive set —
            // its width is the platform's and no member may take it —
            // so the *local* type a position is held in names it.
            Scalar::Size => swift::Type::named("Int"),
        }
    }

    fn named(name: &str) -> Self::Type {
        swift::Type::named(name)
    }

    /// `Void`, which a codec function that answers nothing never
    /// writes: [`Swift::routine`] leaves the arrow off instead.
    fn nothing() -> Self::Type {
        swift::Type::named("Void")
    }

    /// The runtime's handles.
    fn handle(kind: Handle) -> Self::Type {
        swift::Type::named(match kind {
            Handle::DHeader => "ZdDHeader",
            Handle::EmHeader => "ZdEmHeader",
            Handle::Pid => "ZdPid",
            Handle::TryConstruct => "ZdTryConstruct",
            Handle::ReaderMark => "ZdReaderMark",
            Handle::PidRead => "ZdPidRead",
            Handle::EmHeaderRead => "ZdEmHeaderRead",
        })
    }

    /// The member already is the code unit — `UInt8` for a `char`,
    /// `UInt16` for a `wchar` — so there is nothing to convert.
    fn code_point(value: Self::Expr, _narrow: bool) -> Self::Expr {
        value
    }

    /// The runtime's policy values: strings and integers where
    /// `runtime/swift` takes them so, and the runtime's own enums for
    /// the enumerated kinds. One arm per constant and no catch-all.
    fn constant(which: Constant) -> Self::Expr {
        match which {
            Constant::WcharWidth2 => plain(swift::Expr::int(2)),
            Constant::WcharWidth4 => plain(swift::Expr::int(4)),
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
            Constant::ParameterMember => Self::dotted("ZdParameterRole", "member"),
            Constant::ParameterOptional => Self::dotted("ZdParameterRole", "optional"),
            Constant::TryConstructDiscard => Self::dotted("ZdTryConstruct", "discard"),
            Constant::TryConstructTrim => Self::dotted("ZdTryConstruct", "trim"),
            Constant::TryConstructUseDefault => Self::dotted("ZdTryConstruct", "useDefault"),
            Constant::StatusOk => Self::dotted("ZdStatus", "ok"),
            Constant::StatusDiscarded => Self::dotted("ZdStatus", "discarded"),
            Constant::StatusImpossibleLength => Self::dotted("ZdStatus", "impossibleLength"),
            Constant::StatusOutOfRange => Self::dotted("ZdStatus", "outOfRange"),
            Constant::StatusTruncated => Self::dotted("ZdStatus", "truncated"),
        }
    }

    /// The unsigned type wide enough for `bits` of a bit set — the
    /// same answer the materialiser's `swift_holder` gives its fields.
    fn unsigned_holder(bits: u16) -> Self::Type {
        swift::Type::Primitive(swift_holder(bits))
    }

    fn value_type(ast: &Ast, id: Id<TypeNode>) -> Self::Type {
        // A type with no Swift form was refused by the materialiser
        // before a codec walk ran; the fallback name makes the
        // impossible fail in the Swift compiler, loudly, rather than
        // compile into a plausible member.
        swift_value_type(ast, id).unwrap_or_else(|_| swift::Type::named("ZdNoSwiftForm"))
    }

    fn type_name(ast: &Ast, id: Id<TypeNode>) -> String {
        type_tag(ast, id, 0)
    }

    /// The entry name the materialiser declared, and a distinct one
    /// per type when its tag walk refuses: the slot keeps two refused
    /// entries from collapsing onto one declaration, which would merge
    /// two map types under one name.
    fn map_entry_name(ast: &Ast, id: Id<TypeNode>) -> String {
        swift_map_entry_name(ast, id).unwrap_or_else(|_| format!("ZdUnresolvedEntry{}", id.slot()))
    }

    /// Nothing: a declared type's Swift name is already flat — the
    /// adapter folds the modules in — and an enumerator lives in its
    /// enum's own scope, which
    /// [`CodecTarget::enumerator_constant`]'s override reaches.
    fn module_prefix(_ast: &Ast, _id: Id<TypeNode>) -> String {
        String::new()
    }

    /// The escape and nothing else, exactly as the materialiser spells
    /// a property: back quotes round a keyword, which do not rename.
    fn escape(name: &str) -> String {
        property_name(name)
    }

    /// The base is the first member `parent` (XTypes 1.3 §7.5.1.4.1.3).
    fn inherited_path(depth: usize) -> Option<String> {
        Some(format!("{}.", property_name("parent")).repeat(depth))
    }

    // -- expressions ---------------------------------------------------

    fn name(text: &str) -> Self::Expr {
        plain(swift::Expr::Name(Ident::new(text)))
    }

    fn int(value: i128) -> Self::Expr {
        plain(swift::Expr::int(value))
    }

    /// The raw tokens the walk escapes through [`Literal::Int`],
    /// rewritten into the tree's own boolean where they are one.
    fn literal(value: Literal) -> Self::Expr {
        let Literal::Int(text) = &value else {
            return plain(swift::Expr::Literal(value));
        };
        match text.as_str() {
            "true" => plain(swift::Expr::Literal(Literal::Bool(true))),
            "false" => plain(swift::Expr::Literal(Literal::Bool(false))),
            _ => plain(swift::Expr::Literal(value)),
        }
    }

    /// A Swift integer literal is arbitrary-precision and adapts to
    /// its context, so nearly every value is the bare number. The one
    /// exception is a value past `Int64.MAX` reaching a *generic*
    /// context — `zdTo`'s value parameter — where an unconstrained
    /// literal defaults to `Int` and overflows at compile time; those
    /// carry their type: `(16045690984833335998 as UInt64)`.
    fn wide_int(value: i128) -> Self::Expr {
        if value > i128::from(i64::MAX) {
            return plain(swift::Expr::Paren(Box::new(swift::Expr::Binary {
                op: "as",
                lhs: Box::new(swift::Expr::int(value)),
                rhs: Box::new(swift::Expr::Name(Ident::new("UInt64"))),
            })));
        }
        plain(swift::Expr::int(value))
    }

    /// The bare number: the context types it.
    fn unsigned_int(value: i128) -> Self::Expr {
        Self::wide_int(value)
    }

    /// The bare number too. A mask complemented with `~` sits beside a
    /// `UInt64` operand, which is the context that types it — Swift's
    /// literals do not default until no context constrains them.
    fn wide_unsigned(value: u64) -> Self::Expr {
        plain(swift::Expr::Literal(Literal::Int(format!("{value}"))))
    }

    /// The number, for the enumerator patterns `zd_raw` is compared
    /// against; an `Int64` operand types it.
    fn wide_signed(value: i128) -> Self::Expr {
        plain(swift::Expr::Literal(Literal::Int(format!("{value}"))))
    }

    /// `v.x` — and `v._d` as it stands: Swift keeps the IDL-facing
    /// spelling, underscore and all.
    fn member(base: &str, field: &str) -> Self::Expr {
        Self::dotted(base, field)
    }

    fn whole(base: &str) -> Self::Expr {
        plain(swift::Expr::Name(Ident::new(base)))
    }

    fn field_of(value: Self::Expr, field: &str) -> Self::Expr {
        plain(swift::Expr::Member {
            value: Box::new(inner(value)),
            name: Ident::new(field),
        })
    }

    /// A header's field, in the spelling `runtime/swift` publishes:
    /// `member_id` is `memberId`.
    fn header_field(value: Self::Expr, field: &str) -> Self::Expr {
        plain(swift::Expr::Member {
            value: Box::new(inner(value)),
            name: Ident::new(&Self::camel(field)),
        })
    }

    /// `v[Int(i)]`: a Swift array subscript takes `Int` and nothing
    /// else, and the walk's counters are the wire's `UInt32`.
    fn index(value: Self::Expr, at: Self::Expr) -> Self::Expr {
        plain(swift::Expr::Index {
            value: Box::new(inner(value)),
            index: Box::new(Self::invoke(None, "Int", vec![inner(at)])),
        })
    }

    /// `UInt32(v.count)`, or the bound where the materialiser fixed
    /// one: `count` is an `Int` and a sequence's count is a `uint32`
    /// on the wire.
    fn count_of(base: &str, fixed: Option<u64>) -> Self::Expr {
        match fixed {
            Some(bound) => plain(swift::Expr::int(i128::from(bound))),
            None => plain(Self::invoke(
                None,
                "UInt32",
                vec![swift::Expr::Member {
                    value: Box::new(swift::Expr::Name(Ident::new(base))),
                    name: Ident::new("count"),
                }],
            )),
        }
    }

    /// The array itself: a Swift array is the elements.
    fn elements(base: &str) -> Self::Expr {
        plain(swift::Expr::Name(Ident::new(base)))
    }

    /// `v.x!` as a place-marked expression: the branch property is a
    /// `T?` and only this backend knows which side of it a site wants.
    fn case_member(base: &str, field: &str) -> Self::Expr {
        SwiftExpr::BranchSlot(swift::Expr::Member {
            value: Box::new(swift::Expr::Name(Ident::new(base))),
            name: Ident::new(field),
        })
    }

    /// `entry.key` and `entry.value`, the two properties the
    /// materialiser gives a map's entry struct.
    fn entry_half(entry: Self::Expr, key: bool) -> Self::Expr {
        plain(swift::Expr::Member {
            value: Box::new(inner(entry)),
            name: Ident::new(if key { "key" } else { "value" }),
        })
    }

    fn absent() -> Self::Expr {
        plain(swift::Expr::Literal(Literal::Null))
    }

    /// Nothing: a Swift `String` and a Swift array own their storage,
    /// and there is no allocation failure for the guard to test.
    fn materialised(_value: Self::Expr) -> Option<Self::Expr> {
        None
    }

    /// `v.x!` — the value a `T?` holds, given that it holds one. Only
    /// ever reached inside a branch the presence test guarded, where
    /// the unwrap cannot trap.
    fn optional_value(_ast: &Ast, _id: Id<TypeNode>, base: &str, field: &str) -> Self::Expr {
        plain(swift::Expr::ForceUnwrap(Box::new(inner(Self::member(
            base, field,
        )))))
    }

    /// `v.x != nil`.
    fn present(base: &str, field: &str) -> Self::Expr {
        plain(swift::Expr::Binary {
            op: "!=",
            lhs: Box::new(inner(Self::member(base, field))),
            rhs: Box::new(swift::Expr::Literal(Literal::Null)),
        })
    }

    /// Nothing on the way in — assigning the member is what makes it
    /// present, `T` promoting to `T?` on its own — and `v.x = nil` on
    /// the way out.
    fn set_present(base: &str, field: &str, present: bool) -> Vec<Self::Stmt> {
        if present {
            return Vec::new();
        }
        vec![swift::Stmt::Assign {
            place: inner(Self::member(base, field)),
            value: swift::Expr::Literal(Literal::Null),
        }]
    }

    /// `v.x = nil` for every case: a branch property is a `T?`, and only
    /// the selected one holds a value. The same statement as an absent
    /// `@optional` member, [`Self::set_present`] with `false`.
    fn clear_union_cases(base: &str, fields: &[String]) -> Vec<Self::Stmt> {
        fields
            .iter()
            .flat_map(|field| Self::set_present(base, field, false))
            .collect()
    }

    /// The member itself: the assignment's promotion is the making of
    /// room.
    fn optional_destination(base: &str, field: &str) -> (Vec<Self::Stmt>, Self::Expr) {
        (Vec::new(), Self::member(base, field))
    }

    /// `true`: an `@external` member is a `ZdExternal` box, and a member on a
    /// cycle needs the same: a Swift struct cannot store itself.
    /// [`crate::adapter::held_externally`] is the rule the adapter and
    /// the defaulter ask as well.
    fn holds_a_cycle_externally() -> bool {
        true
    }

    /// `v.x.value`, or `v.x!.value` where the box is also `@optional`:
    /// the member's codec takes the value, not the box.
    fn external_value(
        _ast: &Ast,
        _id: Id<TypeNode>,
        base: &str,
        field: &str,
        optional: bool,
    ) -> Self::Expr {
        let member = inner(Self::member(base, field));
        let boxed = if optional {
            swift::Expr::ForceUnwrap(Box::new(member))
        } else {
            member
        };
        plain(swift::Expr::Member {
            value: Box::new(boxed),
            name: Ident::new("value"),
        })
    }

    /// The marked place; `Swift::assemble_assign` routes the decoded
    /// value through the `ZdExternal(..)` constructor, and where the
    /// member is also `@optional` the box promotes to `T?` on its own.
    fn external_destination(
        base: &str,
        field: &str,
        _optional: bool,
    ) -> (Vec<Self::Stmt>, Self::Expr) {
        (
            Vec::new(),
            SwiftExpr::ExternalSlot(inner(Self::member(base, field))),
        )
    }

    /// `v.x != nil`: the optional box is the absence, with nothing
    /// else to carry a second one.
    fn external_present(base: &str, field: &str) -> Self::Expr {
        Self::present(base, field)
    }

    /// Nothing on the way in, `v.x = nil` on the way out.
    fn external_set_present(base: &str, field: &str, present: bool) -> Vec<Self::Stmt> {
        Self::set_present(base, field, present)
    }

    /// `v.x!.value` for a branch: the arm's property is a
    /// `ZdExternal<T>?` and the branch's codec takes the value.
    fn external_case_value(_ast: &Ast, _id: Id<TypeNode>, base: &str, field: &str) -> Self::Expr {
        plain(swift::Expr::Member {
            value: Box::new(inner(Self::case_member(base, field))),
            name: Ident::new("value"),
        })
    }

    /// The marked place, exactly as [`Swift::external_destination`]
    /// answers for a structure member — the branch's bare property,
    /// routed through the box constructor.
    fn external_case_destination(base: &str, field: &str) -> (Vec<Self::Stmt>, Self::Expr) {
        (
            Vec::new(),
            SwiftExpr::ExternalSlot(inner_place(Self::case_member(base, field))),
        )
    }

    /// The operators as given — except the walk's `=`, which is kept
    /// apart so [`Swift::perform`] can route the place through the
    /// slot markers.
    ///
    /// `=`'s right side is already a safe place for a bare `try` —
    /// assignment is the one operator Swift exempts — so `lhs`/`rhs`
    /// there are handed to `inner` untouched and whatever `try` they
    /// carry stays exactly where it is. Every other operator is one
    /// Swift does not exempt, so a `try` `Self::take_try` finds on
    /// either side is hoisted to wrap the whole `Binary` instead of
    /// being left where `swiftc` refuses it.
    fn binary(op: &'static str, lhs: Self::Expr, rhs: Self::Expr) -> Self::Expr {
        if op == "=" {
            return SwiftExpr::Assign(Box::new(lhs), Box::new(inner(rhs)));
        }
        let (lhs, lhs_throws) = Self::take_try(inner(lhs));
        let (rhs, rhs_throws) = Self::take_try(inner(rhs));
        let combined = swift::Expr::Binary {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        };
        Self::maybe_try(combined, lhs_throws || rhs_throws)
    }

    fn unary(op: &'static str, value: Self::Expr) -> Self::Expr {
        let (value, throws) = Self::take_try(inner(value));
        let combined = swift::Expr::Unary {
            op,
            value: Box::new(value),
        };
        Self::maybe_try(combined, throws)
    }

    fn paren(value: Self::Expr) -> Self::Expr {
        let (value, throws) = Self::take_try(inner(value));
        let combined = swift::Expr::Paren(Box::new(value));
        Self::maybe_try(combined, throws)
    }

    /// Every conversion is a call; the module documentation carries
    /// why. Two rewrites: a `bool` discriminator's integer label
    /// becomes the boolean literal, and the walk's `signed char` — the
    /// C spelling of a map key's signed order — is
    /// `Int8(bitPattern: ..)`, which reads a `UInt8` key as the two's
    /// complement the corpus was recorded under.
    fn cast(type_: Self::Type, value: Self::Expr) -> Self::Expr {
        if type_ == swift::Type::named(SIGNED_CHAR) {
            return plain(swift::Expr::Invoke {
                receiver: None,
                name: Ident::new("Int8"),
                args: vec![swift::Argument {
                    label: Some(Ident::new("bitPattern")),
                    value: inner(value),
                }],
            });
        }
        if type_ == swift::Type::Primitive(swift::Primitive::Bool) {
            if let SwiftExpr::Plain(swift::Expr::Literal(Literal::Int(text))) = &value {
                return plain(swift::Expr::Literal(Literal::Bool(text != "0")));
            }
        }
        plain(Self::convert(type_, inner(value)))
    }

    /// `false` before `true`, spelled as the conjunction — Swift's `<`
    /// takes no booleans. The same hoisting `binary` does: `&&` is not
    /// the assignment operator either.
    fn bool_less(left: Self::Expr, right: Self::Expr) -> Self::Expr {
        let (left, left_throws) = Self::take_try(inner(left));
        let (right, right_throws) = Self::take_try(inner(right));
        let combined = swift::Expr::Binary {
            op: "&&",
            lhs: Box::new(swift::Expr::Paren(Box::new(swift::Expr::Binary {
                op: "==",
                lhs: Box::new(left),
                rhs: Box::new(swift::Expr::Literal(Literal::Bool(false))),
            }))),
            rhs: Box::new(right),
        };
        Self::maybe_try(combined, left_throws || right_throws)
    }

    /// `v.rawValue`. `adapter/swift.rs` declares an enumeration as
    /// `enum Name: Int32` with each enumerator's declared value as its
    /// raw value, and a Swift enum has no `<` of its own: comparing two
    /// keys of the enumeration directly does not compile ("cannot convert
    /// value of type 'Colour' to expected argument type 'Int32'",
    /// measured in the matrix refresh of 2026-10-01 on `map-key-enum`
    /// under the Fast DDS and OpenDDS dialects). The raw value is
    /// [`zerodds_ast_layout::MapKeyOrdering::EnumeratorValue`]'s order.
    /// The union overlay asks the same seam for every integer arm, and
    /// `.rawValue` on an `Int32` does not compile: `combined-everything`
    /// under the Cyclone dialect generated `v.whole!.rawValue`. A type
    /// that is not an enumeration passes through.
    fn enumerator_ordinal(ast: &Ast, id: Id<TypeNode>, value: Self::Expr) -> Self::Expr {
        if !super::resolves_to_enumeration(ast, id) {
            return value;
        }
        let (value, throws) = Self::take_try(inner(value));
        Self::maybe_try(
            swift::Expr::Member {
                value: Box::new(value),
                name: Ident::new("rawValue"),
            },
            throws,
        )
    }

    /// The value, as a value: a slot marker crossing a call boundary
    /// is the branch unwrap materialising.
    fn handoff(value: Self::Expr, _form: Handoff) -> Self::Expr {
        plain(inner(value))
    }

    /// A call to a file-level function — a generated codec or
    /// defaulter. `strcmp` is the runtime's `zdStrcmp`, which spells
    /// C11 §7.24.4's unsigned-octet order over the UTF-8 views.
    ///
    /// Every other callee is another codec function this walk emits —
    /// a member's own encoder or decoder, called recursively — and
    /// every one of those is `throws` (see [`Swift::routine`]), so the
    /// call is wrapped here, at the one place that builds it, rather
    /// than trusted to whoever calls [`Swift::call`].
    fn call(callee: &str, args: Vec<Self::Expr>) -> Self::Expr {
        let arguments: Vec<swift::Expr> = args.into_iter().map(inner).collect();
        if callee == "strcmp" {
            return plain(Self::invoke(None, "zdStrcmp", arguments));
        }
        plain(swift::Expr::Try(Box::new(Self::invoke(
            None, callee, arguments,
        ))))
    }

    /// `w.writeU32(x)` — a method on the writer or the reader, `try`
    /// in front of it exactly where `Self::runtime_method_throws`
    /// says the method refuses.
    fn runtime(receiver: &str, method: &str, args: Vec<Self::Expr>) -> Self::Expr {
        let invoke = Self::invoke(
            Some(swift::Expr::Name(Ident::new(receiver))),
            &Self::camel(method),
            args.into_iter().map(inner).collect(),
        );
        Self::maybe_try(invoke, Self::runtime_method_throws(method))
    }

    /// `runtime/swift` carries the three HDDS axes: `beginPackedElement`
    /// / `finishPackedElement` on the writer and
    /// `readerBeginPackedElement` / `readerFinishPackedElement` on the
    /// reader, the DHEADER pair for a sequence element's own length, and
    /// `writeWstringUtf8Octets` / `writeWstringUtf8OctetsBounded` /
    /// `readWstringUtf8Octets`.
    fn carries_hdds_axes() -> bool {
        true
    }

    /// A free function of the runtime: `no_dheader` is `zdNoDheader`,
    /// `pid_is_user_member` is `zdPidIsUserMember`.
    fn runtime_static(method: &str, args: Vec<Self::Expr>) -> Self::Expr {
        plain(Self::invoke(
            None,
            &Self::free_name(method),
            args.into_iter().map(inner).collect(),
        ))
    }

    // -- statements ----------------------------------------------------

    /// `var name: T = value` — the type written, so a reader of the
    /// generated file sees the width and an unsuffixed literal takes
    /// it; `var name: T` where the walk asked for no value, leaning on
    /// Swift's definite initialisation.
    fn declare(type_: Self::Type, name: &str, value: Option<Self::Expr>) -> Self::Stmt {
        swift::Stmt::Var {
            name: Ident::new(name),
            type_: Some(type_),
            value: value.map(inner),
        }
    }

    /// An expression evaluated for its effect — or, where the walk
    /// built an assignment, the assignment routed through the place's
    /// marker.
    fn perform(value: Self::Expr) -> Self::Stmt {
        if let SwiftExpr::Assign(place, value) = value {
            return Self::assemble_assign(*place, *value);
        }
        swift::Stmt::Expr(inner(value))
    }

    /// `return v` — a Swift decoder answers with the sample it built.
    fn give_back(value: Option<Self::Expr>) -> Self::Stmt {
        swift::Stmt::Return(Some(match value {
            Some(value) => inner(value),
            None => swift::Expr::Name(Ident::new("v")),
        }))
    }

    fn when(
        condition: Self::Expr,
        then: Self::Block,
        otherwise: Option<Self::Block>,
    ) -> Self::Stmt {
        swift::Stmt::If {
            condition: inner(condition),
            then,
            otherwise: otherwise.filter(|body| !body.is_empty()),
        }
    }

    /// `for i in 0..<bound`: the counter is the bound's own type,
    /// which is what keeps every comparison in the body homogeneous.
    fn count_up(counter: &str, bound: Self::Expr, body: Self::Block) -> Self::Stmt {
        swift::Stmt::ForRange {
            counter: Ident::new(counter),
            bound: inner(bound),
            body,
        }
    }

    fn until(condition: Self::Expr, body: Self::Block) -> Self::Stmt {
        swift::Stmt::While {
            condition: inner(condition),
            body,
        }
    }

    fn leave() -> Self::Stmt {
        swift::Stmt::Break
    }

    fn note(doc: Doc) -> Self::Stmt {
        swift::Stmt::Comment(doc)
    }

    /// `_ = expr` — Swift's spelling of a value read for its effect.
    fn drop_value(value: Self::Expr) -> Self::Stmt {
        swift::Stmt::Assign {
            place: swift::Expr::Name(Ident::new("_")),
            value: inner(value),
        }
    }

    /// `_ = name` — Swift does not warn on an unread parameter, but
    /// the walk also marks locals a framing leaves unread, and those
    /// do warn.
    fn mark_used(name: &str) -> Vec<Self::Stmt> {
        vec![swift::Stmt::Assign {
            place: swift::Expr::Name(Ident::new("_")),
            value: swift::Expr::Name(Ident::new(name)),
        }]
    }

    /// `place = try zd_dec_T(r, behaviour)` — through the place's
    /// marker: a decoder answers with its value, an `@external` place
    /// wraps it in the box, and a sequence slot appends it. Every one
    /// of those is a place a bare `try` sits safely in front of a call
    /// argument or an assignment's right side, which is what
    /// `callee` — another generated decoder, always `throws` — needs.
    fn read_call(
        callee: &str,
        place: Self::Expr,
        _form: Handoff,
        behaviour: Self::Expr,
    ) -> Self::Stmt {
        let decoded = swift::Expr::Try(Box::new(Self::invoke(
            None,
            callee,
            vec![swift::Expr::Name(Ident::new("r")), inner(behaviour)],
        )));
        Self::assemble_assign(place, decoded)
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

    fn block(statements: Vec<Self::Stmt>) -> Self::Block {
        statements
    }

    fn into_statements(block: Self::Block) -> Vec<Self::Stmt> {
        block
    }

    /// Room, in the two shapes Swift has. A sequence's elements are
    /// *appended* — [`Swift::sequence_element`] marks the slot — so
    /// its room is the empty array and a `reserveCapacity`. A map's
    /// entries are written into, both halves per slot, so its room is
    /// real entries: `[Entry](repeating: zd_def_Entry(), count: n)`,
    /// over the defaulter `default_swift` emits for the entry struct.
    fn make_room(
        base: &str,
        element: Self::Type,
        count: Self::Expr,
        assigned_whole: bool,
        fill: Vec<Self::Stmt>,
    ) -> Vec<Self::Stmt> {
        let count = inner(count);
        if assigned_whole {
            let mut out = vec![
                swift::Stmt::Assign {
                    place: swift::Expr::Name(Ident::new(base)),
                    value: swift::Expr::ArrayLit(Vec::new()),
                },
                swift::Stmt::Expr(Self::invoke(
                    Some(swift::Expr::Name(Ident::new(base))),
                    "reserveCapacity",
                    vec![Self::invoke(None, "Int", vec![count])],
                )),
            ];
            out.extend(fill);
            return out;
        }
        let repeated = match &element {
            swift::Type::Named(name) => {
                Self::invoke(None, &format!("zd_def_{}", name.as_str()), Vec::new())
            }
            // Only a map's entry struct reaches the written-into form,
            // and its type is always a name; anything else is a walk
            // this backend does not know, made loud in the compiler.
            _ => Self::invoke(None, "zd_def_unsupported_room", Vec::new()),
        };
        let mut out = vec![swift::Stmt::Assign {
            place: swift::Expr::Name(Ident::new(base)),
            value: swift::Expr::RepeatingArray {
                element: Box::new(element),
                repeated: Box::new(repeated),
                count: Box::new(Self::invoke(None, "Int", vec![count])),
            },
        }];
        out.extend(fill);
        out
    }

    /// `v = Array(v.prefix(Int(count)))` — §7.3.1.2.1.12.3's TRIM,
    /// applied after every element has been read.
    fn keep_first(base: &str, count: Self::Expr) -> Vec<Self::Stmt> {
        vec![swift::Stmt::Assign {
            place: swift::Expr::Name(Ident::new(base)),
            value: Self::invoke(
                None,
                "Array",
                vec![Self::invoke(
                    Some(swift::Expr::Name(Ident::new(base))),
                    "prefix",
                    vec![Self::invoke(None, "Int", vec![inner(count)])],
                )],
            ),
        }]
    }

    /// The marked slot: the element decoders append, because a Swift
    /// array has no uninitialised slot to write into.
    fn sequence_element(
        base: &str,
        _index: Self::Expr,
        _element: Self::Type,
    ) -> (Vec<Self::Stmt>, Self::Expr, Vec<Self::Stmt>) {
        (
            Vec::new(),
            SwiftExpr::AppendSlot(swift::Expr::Name(Ident::new(base))),
            Vec::new(),
        )
    }

    /// A local the body writes into. A generated struct has no
    /// zero-argument initialiser, so the storage is its Table 9 value,
    /// through the defaulter named after the type.
    fn storage(type_: Self::Type, name: &str) -> Self::Stmt {
        match &type_ {
            swift::Type::Named(type_name) => swift::Stmt::Var {
                name: Ident::new(name),
                type_: None,
                value: Some(Self::invoke(
                    None,
                    &format!("zd_def_{}", type_name.as_str()),
                    Vec::new(),
                )),
            },
            _ => swift::Stmt::Var {
                name: Ident::new(name),
                type_: Some(type_),
                value: None,
            },
        }
    }

    // -- assembly ------------------------------------------------------

    /// `func zd_enc_T(_ w: ZdWriter, _ v: T) throws` and
    /// `func zd_dec_T(_ r: ZdReader, _ zd_tc: ZdTryConstruct) throws -> T`.
    ///
    /// A forward declaration is answered with nothing: a Swift
    /// file-scope name is in scope for the whole file however far down
    /// it is written.
    ///
    /// Every routine is `throws`, whether or not this particular one
    /// happens to call something that refuses: a routine's own body can
    /// change — a member added, a bound relaxed — without this
    /// declaration having to change with it, and a caller one call away
    /// from a refusal and a caller ten calls away are the same call
    /// either way.
    fn routine(spec: Routine<Self>) -> Option<Self::Decl> {
        let body = spec.body?;
        let underscore = |name: &str, type_: swift::Type| swift::Param {
            label: Ident::new("_"),
            internal_name: Some(Ident::new(name)),
            type_,
        };
        let mut params = vec![underscore(
            if spec.writing { "w" } else { "r" },
            swift::Type::named(if spec.writing { "ZdWriter" } else { "ZdReader" }),
        )];
        if spec.writing {
            params.push(underscore("v", spec.sample.clone()));
        }
        for (type_, name) in spec.extra {
            params.push(underscore(&name, type_));
        }
        let (returns, body) = if spec.writing {
            (None, body)
        } else {
            // The sample starts as Table 9's value, through the
            // generated defaulter, so every position the stream does
            // not carry is already what the clause says it is.
            let defaulter = crate::default_swift::defaulter_from_decoder(&spec.name);
            let mut statements = vec![swift::Stmt::Var {
                name: Ident::new("v"),
                type_: None,
                value: Some(Self::invoke(None, &defaulter, Vec::new())),
            }];
            statements.extend(body);
            statements.push(swift::Stmt::Return(Some(swift::Expr::Name(Ident::new(
                "v",
            )))));
            (Some(spec.sample), statements)
        };
        Some(swift::Decl::Func(swift::FuncDecl {
            doc: spec.doc,
            // `public`, like the aggregates this file declares beside
            // it. Swift's default is `internal` — the module — so a
            // generated file compiled as its own module offered a
            // `public struct Max` that no consumer could encode. The
            // probe harness never saw it: it compiles its `main` into
            // the same module.
            visibility: zerodds_lang_cst::common::Visibility::Public,
            name: Ident::new(&spec.name),
            params,
            throws: true,
            returns,
            body,
        }))
    }

    /// `Flags(rawValue: <holder>)` — an `OptionSet` has no conversion
    /// operator, so the 64-bit read narrows through the declared
    /// holder and enters by the raw-value initialiser.
    fn bitmask_value(ast: &Ast, id: Id<TypeNode>, value: Self::Expr) -> Self::Expr {
        let holder = swift::Type::Primitive(swift_holder(ast.effective_bit_bound(id)));
        plain(swift::Expr::Invoke {
            receiver: None,
            name: Ident::new(&swift_type_name(ast, id)),
            args: vec![swift::Argument {
                label: Some(Ident::new("rawValue")),
                value: Self::convert(holder, inner(value)),
            }],
        })
    }

    /// A call to the generated defaulter, which is Table 9 once per
    /// type rather than once per position — routed through the place's
    /// marker exactly as [`Swift::read_call`]'s answer is.
    ///
    /// # Errors
    /// Never; the signature matches the emitters around it.
    fn default_into(
        ast: &Ast,
        id: Id<TypeNode>,
        place: Self::Expr,
    ) -> Result<Self::Stmt, EncodeError> {
        let defaulted = Self::invoke(
            None,
            &crate::default_swift::defaulter_name(ast, id),
            Vec::new(),
        );
        Ok(Self::assemble_assign(place, defaulted))
    }

    /// # Errors
    /// As [`crate::default_swift::member_default`]: a `@default` this
    /// column cannot spell is refused rather than silently replaced by
    /// Table 9's zero.
    fn member_default(
        ast: &Ast,
        node: &MemberNode,
        type_: Id<TypeNode>,
        place: Self::Expr,
    ) -> Result<Vec<Self::Stmt>, EncodeError> {
        crate::default_swift::member_default(ast, node, type_, inner_place(place))
    }

    /// `Mode.IDLE` — the enum's own scope, which is where the
    /// materialiser declared the case. The trait's default answers a
    /// bare prefixed name, which in Swift names nothing.
    fn enumerator_constant(ast: &Ast, id: Id<TypeNode>, enumerator: &str) -> Self::Expr {
        plain(swift::Expr::Member {
            value: Box::new(swift::Expr::Name(Ident::new(&swift_type_name(ast, id)))),
            name: Ident::new(&property_name(enumerator)),
        })
    }
}

/// The expression as a *place*: the branch slot stays the property —
/// assignment promotes — where [`inner`] would unwrap it.
/// zerodds-lint: recursion-depth 2 (an Assign holds one place, never another Assign)
fn inner_place(expr: SwiftExpr) -> swift::Expr {
    match expr {
        SwiftExpr::Plain(e)
        | SwiftExpr::BranchSlot(e)
        | SwiftExpr::ExternalSlot(e)
        | SwiftExpr::AppendSlot(e) => e,
        SwiftExpr::Assign(place, _) => inner_place(*place),
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
/// generated Swift held no `zd_over` check at all, so `a` accepted a
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
/// and not the Swift spelling, structurally for a collection and the
/// flattened Swift name for a declaration — the same scheme the D and
/// Go columns carry, for the reasons their `type_tag`s state. A back
/// quote the escape put round a keyword-named type is stripped: it is
/// not an identifier character, and the bare spelling cannot collide
/// with anything else in the file precisely because the bare spelling
/// is reserved.
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
                    // No number rather than a zero: the name has to
                    // differ from a `[0]` array's, and the materialiser
                    // reports the dimension that would not evaluate.
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
        // An alias is followed through: two IDL names for one shape are
        // one wire form, and two codecs for it would be two chances to
        // differ.
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
        | TypeKind::Native => swift_type_name(ast, id).replace('`', ""),
        TypeKind::Any => "any".to_string(),
        TypeKind::ValueBase => "valuebase".to_string(),
        TypeKind::ObjectRef { .. } => "objectref".to_string(),
        TypeKind::AnnotationType { .. } => "annotation".to_string(),
    }
}
