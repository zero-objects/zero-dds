// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Lua backend of the shared codec walk.
//!
//! DDS-XTypes 1.3 §7.4 from the clause, the same as `runtime/c`,
//! `runtime/python` and the rest do, and none of them calls another.
//! Where two columns disagree on an octet, one has misread the
//! specification — which is the only reason to have another.
//!
//! # The shapes this column holds a value in
//!
//! `adapter/lua.rs` is the authority and this file spells nothing it
//! does not declare:
//!
//! - a struct, a union, a bitset, a sequence, an array and a map are
//!   all **tables** — a sequence 1-based, a map an array of
//!   `{ key = …, value = … }` pairs;
//! - a `char` and a `wchar` are **integers** holding the code point,
//!   which is why [`Lua::code_point`] is the identity and a character
//!   case label stays the number it arrived as;
//! - an `unsigned long long` is the 64-bit two's-complement **bit
//!   pattern** in a Lua integer, which is why [`Lua::unsigned_less`]
//!   answers `math.ult` at eight octets — Lua's `<` on two patterns is
//!   signed and sorted `16045690984833335998` after `7`;
//! - a `long double` and a `fixed` are decimal **strings**, converted
//!   at the octet by `runtime/lua`;
//! - an absent `@optional` member is an absent **key**: `v.m ~= nil`
//!   is the presence test and `v.m = nil` the removal, because in Lua
//!   the absence of a value is the absence of the key.
//!
//! # The four decisions this column turns on
//!
//! **Codecs live in one table.** Every generated routine is
//! `function ZD.zd_enc_T(w, v) … end` and every call is `ZD.zd_enc_T`,
//! because a chunk holds at most 200 locals and a corpus case's codecs,
//! defaulters and dumpers together would cross that as `local function`s.
//! A table lookup also resolves at the call and not at the definition,
//! so a recursive or mutually recursive type needs no forward
//! declaration — [`Lua::routine`] answers `None` for one, exactly as
//! the Python backend does. The caller binds `local ZD = {}` before the
//! declarations, the same contract `adapter::lua::lua_items` has for `M`.
//!
//! **An assignment is a statement.** `a = b` is not a Lua expression.
//! The shared walk builds one through `T::perform(T::binary("=", …))`
//! at every assignment site, so [`Lua::perform`] recognises that shape
//! and answers [`lua::Stat::Assign`]; everything else it receives is a
//! call, which is the one expression Lua's grammar takes as a statement.
//!
//! **A counted loop stays zero-based and the subscript adds one.** The
//! walk counts `0 .. bound` exclusive and compares counters against
//! ranks; a backend that moved the loop to `1 .. bound` would have to
//! move the rank arithmetic with it. So [`Lua::count_up`] writes
//! `for i = 0, bound - 1` and [`Lua::index`] writes `t[(i) + 1]`, and
//! the 1-based convention is decided in exactly one method.
//!
//! **The boolean spellings are Lua's.** `&&`, `||`, `!` and `!=` become
//! `and`, `or`, `not` and `~=`; `false` before true — the raw token the
//! walk escapes through `Literal::Int` — is rewritten to the literal,
//! as the Python backend does for `False`.
//!
//! # What Lua cannot check, said plainly
//!
//! Nothing in a generated chunk declares a type, so a wrong shape here
//! is not a compile error anywhere — it is a wrong octet in the
//! measurement or an error at run time. That is why this backend leans
//! on the two loud failure modes Lua does have: a call through a name
//! `ZD` does not hold reports the name at the line, and `runtime/lua`
//! range-checks every value at the octet.

use super::{CodecTarget, Constant, Handle, Handoff, Routine, Scalar};
use crate::adapter::lua::declared_place;
use crate::encode::EncodeError;
use zerodds_ast::{Ast, CharWidth, MemberNode, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::{Doc, Ident, Literal};
use zerodds_lang_cst::lua::{self, Expr, Place, Stat};

/// The Lua backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lua;

/// The table the generated codecs, defaulters and dumpers live in.
///
/// The caller binds `local ZD = {}` ahead of the declarations — the
/// same contract `adapter::lua::lua_items` states for `M`.
pub const CODECS: &str = "ZD";

/// The table the runtime is required into: `local zdw =
/// require("zerodds_wire")`. Prefixed like every other generated name,
/// so no IDL identifier the corpus declares reaches it.
pub const RUNTIME: &str = "zdw";

/// How deep a chain of typedefs and collections is followed.
const MAX_TYPE_DEPTH: usize = 64;

/// The type name [`CodecTarget::named`] is handed for C's signed
/// `char`. `emit/encode.rs`'s `map_key_less` writes it where a `map`'s
/// key is a `char` under `MapKeyOrdering::SignedInteger`; this column
/// holds the code point as an integer, so the reading becomes the
/// arithmetic `((cp + 128) % 256) - 128` — Lua's `%` is a floored
/// modulo, so the wrap is exact for the whole octet.
const SIGNED_CHAR: &str = "signed char";

/// What [`CodecTarget::map_entry_name`] answers with: not a type this
/// column declares but a marker [`Lua::storage`] matches on, because a
/// map entry is a plain `{ key = …, value = … }` table.
const MAP_ENTRY: &str = "_zd_map_entry";

/// The nominal type a local is annotated with — which in Lua is
/// nothing at all, so the type is a tag this backend keeps for its own
/// two decisions: whether a cast target is `boolean` (a union label
/// becomes `true`/`false`), and whether a storage request is the map
/// entry marker. Everything else is carried through untouched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LuaType(pub String);

impl Lua {
    /// A possibly dotted name as a place.
    fn place_from_name(text: &str) -> Place {
        let mut segments = text.split('.');
        let mut place = Place::name(segments.next().unwrap_or(text));
        for segment in segments {
            place = place.field(segment);
        }
        place
    }

    /// An expression read back as the place it names.
    ///
    /// The walk only ever indexes and dereferences values it built as
    /// names, subscripts and fields — all places — so the fallback arm
    /// is the grammar's own escape: a parenthesised expression is a
    /// legal base for a field or an index (Lua 5.4 reference §3.4).
    fn place_of(value: Expr) -> Place {
        match value {
            Expr::Place(place) => place,
            Expr::Paren(inner) => match *inner {
                Expr::Place(place) => place,
                other => Place::Paren(Box::new(other)),
            },
            other => Place::Paren(Box::new(other)),
        }
    }

    /// A call on the runtime table: `zdw.<method>(args…)`.
    fn runtime_call(method: &str, args: Vec<Expr>) -> Expr {
        Expr::Call {
            func: Box::new(Place::name(RUNTIME).field(method)),
            args,
        }
    }

    /// A call on the codec table: `ZD.<callee>(args…)`.
    fn codec_call(callee: &str, args: Vec<Expr>) -> Expr {
        Expr::Call {
            func: Box::new(Place::name(CODECS).field(callee)),
            args,
        }
    }

    /// A string literal.
    fn text(value: &str) -> Expr {
        Expr::string(value)
    }

    /// A reader's body, with the sample it answers with around it:
    /// `local v = ZD.zd_def_T()` … `return v`. Table 9's value through
    /// the generated defaulter, so every position the stream does not
    /// carry is already what the clause says before an octet is read;
    /// the defaulter and the decoder are keyed the same way and differ
    /// in three letters.
    fn reading_body(name: &str, body: Vec<Stat>) -> Vec<Stat> {
        let defaulter = crate::default_lua::defaulter_from_decoder(name);
        let mut statements = vec![Stat::Local {
            doc: Doc::none(),
            name: Ident::new("v"),
            value: Some(Self::codec_call(&defaulter, Vec::new())),
        }];
        statements.extend(body);
        statements.push(Stat::Return(Some(Expr::place(Place::name("v")))));
        statements
    }
}

impl CodecTarget for Lua {
    type Expr = Expr;
    type Stmt = Stat;
    type Block = Vec<Stat>;
    type Type = LuaType;
    type Decl = Stat;

    // -- types --------------------------------------------------------

    /// The tag and nothing else: Lua declares no types. `boolean` is
    /// the one tag [`Self::cast`] acts on.
    fn scalar(kind: Scalar) -> Self::Type {
        LuaType(
            match kind {
                Scalar::Bool => "boolean",
                // A char member holds the code point as an integer —
                // `adapter/lua.rs`'s mapping.
                Scalar::Char => "integer",
                Scalar::Int8
                | Scalar::UInt8
                | Scalar::Int16
                | Scalar::UInt16
                | Scalar::Int32
                | Scalar::UInt32
                | Scalar::Int64
                | Scalar::UInt64
                | Scalar::Index
                | Scalar::Size => "integer",
                Scalar::Float | Scalar::Double => "number",
                // The decimal text; `runtime/lua` converts at the octet.
                Scalar::LongDouble => "string",
            }
            .to_string(),
        )
    }

    fn named(name: &str) -> Self::Type {
        LuaType(name.to_string())
    }

    fn nothing() -> Self::Type {
        LuaType("nil".to_string())
    }

    /// Every runtime handle is a plain table.
    fn handle(_kind: Handle) -> Self::Type {
        LuaType("table".to_string())
    }

    /// The member holds the code point already, so there is nothing to
    /// convert in either direction.
    fn code_point(value: Self::Expr, _narrow: bool) -> Self::Expr {
        value
    }

    /// The runtime's policy values, in the spelling `runtime/lua`
    /// matches — the same strings `runtime/python` takes, because both
    /// spell `WstringLengthPolicy::as_str` and its siblings. One arm
    /// per constant and no catch-all, so a policy this column has not
    /// implemented is a compile error here rather than a plausible
    /// value.
    fn constant(which: Constant) -> Self::Expr {
        match which {
            // The octet count itself: `write_wchar` takes the number.
            Constant::WcharWidth2 => Expr::int(2),
            Constant::WcharWidth4 => Expr::int(4),
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
            Constant::ParameterMember => Self::text("member"),
            Constant::ParameterOptional => Self::text("optional"),
            Constant::TryConstructDiscard => Self::text("discard"),
            Constant::TryConstructTrim => Self::text("trim"),
            Constant::TryConstructUseDefault => Self::text("use-default"),
            Constant::StatusOk => Self::text("ok"),
            Constant::StatusDiscarded => Self::text("discarded"),
            Constant::StatusImpossibleLength => Self::text("impossible-length"),
            Constant::StatusOutOfRange => Self::text("out-of-range"),
            Constant::StatusTruncated => Self::text("truncated"),
        }
    }

    /// A Lua integer, at every bit bound: 64 bits hold every
    /// `@bit_bound` a set can declare.
    fn unsigned_holder(_bits: u16) -> Self::Type {
        LuaType("integer".to_string())
    }

    /// Whether `left` is less than `right` as unsigned integers.
    ///
    /// At eight octets the held value is the two's-complement bit
    /// pattern and Lua's `<` is signed, so the answer is `math.ult` —
    /// Lua's own unsigned comparison. Narrower widths hold the value
    /// itself, non-negative, and the default `<` is right.
    fn unsigned_less(octets: u8, left: Self::Expr, right: Self::Expr) -> Self::Expr {
        if octets < 8 {
            return Self::binary("<", left, right);
        }
        Expr::Call {
            func: Box::new(Place::name("math").field("ult")),
            args: vec![left, right],
        }
    }

    /// `false` before `true`. Lua's `<` refuses two booleans outright —
    /// "attempt to compare two boolean values" — so the order is
    /// spelled as `not (left) and (right)`, which is false-before-true
    /// exactly.
    fn bool_less(left: Self::Expr, right: Self::Expr) -> Self::Expr {
        Expr::Binary {
            op: "and",
            lhs: Box::new(Expr::Unary {
                op: "not",
                value: Box::new(Expr::Paren(Box::new(left))),
            }),
            rhs: Box::new(Expr::Paren(Box::new(right))),
        }
    }

    fn value_type(ast: &Ast, id: Id<TypeNode>) -> Self::Type {
        value_type_of(ast, id, 0)
    }

    fn type_name(ast: &Ast, id: Id<TypeNode>) -> String {
        type_tag(ast, id, 0)
    }

    /// A map entry is a plain `{ key = …, value = … }` table; the name
    /// is a marker [`Self::storage`] matches on and nothing declares.
    fn map_entry_name(_ast: &Ast, _id: Id<TypeNode>) -> String {
        MAP_ENTRY.to_string()
    }

    /// Nothing: the declared paths this backend writes go through
    /// [`Self::enumerator_constant`] and the defaulters, both rooted at
    /// `M`, and a prefix would be a second answer to the same question.
    fn module_prefix(_ast: &Ast, _id: Id<TypeNode>) -> String {
        String::new()
    }

    /// The identifier, unchanged. Lua renames nothing: a keyword member
    /// keeps its name and only the syntax it is reached by changes —
    /// `t["end"]` instead of `t.end` — which `lua::Place::field` and
    /// `lua::table_key` decide at the use.
    fn escape(name: &str) -> String {
        name.to_string()
    }

    // -- expressions ---------------------------------------------------

    fn name(text: &str) -> Self::Expr {
        Expr::place(Self::place_from_name(text))
    }

    fn int(value: i128) -> Self::Expr {
        Expr::int(value)
    }

    /// An integer literal that survives its own width. A Lua decimal
    /// numeral above 2^63 − 1 denotes a *float* (reference §3.1) and a
    /// hexadecimal one wraps to the exact bit pattern, so a union label
    /// like `16045690984833335998` is written `0xDEADBEEFCAFEBABE`-style
    /// — the same reading the member itself holds. The same care covers
    /// −2^63 itself: `-9223372036854775808` is unary minus over a
    /// numeral that is already a *float* by the time the minus applies,
    /// so the minimum goes out as its own hex pattern too.
    fn wide_int(value: i128) -> Self::Expr {
        if value == i128::from(i64::MIN) {
            return Expr::Literal(Literal::Int("0x8000000000000000".to_string()));
        }
        if i64::try_from(value).is_ok() {
            return Expr::int(value);
        }
        if let Ok(unsigned) = u64::try_from(value) {
            return Expr::Literal(Literal::Int(format!("0x{unsigned:016X}")));
        }
        // No Lua integer holds it; the masked pattern is what the
        // 64-bit member the label is compared against would hold.
        #[allow(clippy::cast_sign_loss)]
        let masked = (value as u64).wrapping_add(0);
        Expr::Literal(Literal::Int(format!("0x{masked:016X}")))
    }

    /// As [`Self::wide_int`], from the unsigned side.
    fn wide_unsigned(value: u64) -> Self::Expr {
        if i64::try_from(value).is_ok() {
            return Expr::int(i128::from(value));
        }
        Expr::Literal(Literal::Int(format!("0x{value:016X}")))
    }

    fn wide_signed(value: i128) -> Self::Expr {
        Self::wide_int(value)
    }

    /// The number, with no suffix: `2u` is not a Lua numeral.
    fn unsigned_int(value: i128) -> Self::Expr {
        Expr::int(value)
    }

    /// A literal, with the two raw tokens the walk escapes through
    /// [`Literal::Int`] read back as the booleans they are. Lua spells
    /// them the same way C does, so this is for the tree's sake rather
    /// than the renderer's: a `Literal::Bool` cannot be mistaken for a
    /// number by anything that walks the statements later.
    fn literal(value: Literal) -> Self::Expr {
        if let Literal::Int(text) = &value {
            match text.as_str() {
                "true" => return Expr::bool(true),
                "false" => return Expr::bool(false),
                _ => {}
            }
        }
        Expr::Literal(value)
    }

    /// A call to a generated codec — or, for the one C name the walk
    /// hands here, to the runtime's byte-wise `strcmp`: Lua's own `<`
    /// on strings goes through `strcoll` and the process's locale, and
    /// a map's entry order must not.
    fn call(callee: &str, args: Vec<Self::Expr>) -> Self::Expr {
        if callee == "strcmp" {
            return Self::runtime_call("strcmp", args);
        }
        Self::codec_call(callee, args)
    }

    /// `zdw.write_u32(w, x)` — the runtime is a table of functions
    /// taking the writer or reader first, so the receiver becomes the
    /// first argument.
    fn runtime(receiver: &str, method: &str, args: Vec<Self::Expr>) -> Self::Expr {
        // `read_pid()` is the walk's name for "read a header in the
        // MEMBER role and open its value block": §7.4.1.2 counts a
        // member's internal alignment from the first octet after its
        // header, and the walk closes the block through
        // `finish_pid_read`. `read_pid_as` is the runtime entry that
        // opens it. The same rewrite the Python backend makes, for the
        // same wire reason.
        if method == "read_pid" && args.is_empty() {
            return Self::runtime(
                receiver,
                "read_pid_as",
                vec![Self::constant(Constant::ParameterMember)],
            );
        }
        let mut all = vec![Self::name(receiver)];
        all.extend(args);
        Self::runtime_call(method, all)
    }

    /// A free function of the runtime — the same table, because the
    /// runtime is one table: `zdw.no_dheader()`,
    /// `zdw.pid_is_user_member(zd_p)`, `zdw.utf8_octets(v)`.
    fn runtime_static(method: &str, args: Vec<Self::Expr>) -> Self::Expr {
        Self::runtime_call(method, args)
    }

    /// `runtime/lua` carries the three HDDS axes: `begin_packed_element` /
    /// `finish_packed_element` on the writer and
    /// `reader_begin_packed_element` / `reader_finish_packed_element` on the
    /// reader, the DHEADER pair for a sequence element's own length, and
    /// `write_wstring_utf8_octets` / `read_wstring_utf8_octets`.
    fn carries_hdds_axes() -> bool {
        true
    }

    /// A field name with dots is a path through the members that hold a base
    /// (`parent.x`, XTypes 1.3 §7.5.1.4.1.3): one field step per segment.
    fn member(base: &str, field: &str) -> Self::Expr {
        let mut place = Place::name(base);
        for segment in field.split('.') {
            place = place.field(segment);
        }
        Expr::place(place)
    }

    /// The base is the first member `parent` (XTypes 1.3 §7.5.1.4.1.3).
    fn inherited_path(depth: usize) -> Option<String> {
        Some("parent.".repeat(depth))
    }

    fn whole(base: &str) -> Self::Expr {
        Expr::place(Place::name(base))
    }

    fn field_of(value: Self::Expr, field: &str) -> Self::Expr {
        Expr::place(Self::place_of(value).field(field))
    }

    /// `t[(i) + 1]` — the one place the 0-based walk meets Lua's
    /// 1-based tables. Every counter stays 0-based (see
    /// [`Self::count_up`]), so every subscript adds one, here and
    /// nowhere else.
    fn index(value: Self::Expr, at: Self::Expr) -> Self::Expr {
        Expr::place(Place::Index {
            base: Box::new(Self::place_of(value)),
            key: Box::new(Expr::Binary {
                op: "+",
                lhs: Box::new(Expr::Paren(Box::new(at))),
                rhs: Box::new(Expr::int(1)),
            }),
        })
    }

    /// `#v`, or the bound where the materialiser fixed one. The length
    /// operator counts the array part's border, which for the
    /// contiguous tables this column builds is the element count.
    fn count_of(base: &str, fixed: Option<u64>) -> Self::Expr {
        match fixed {
            Some(bound) => Expr::int(i128::from(bound)),
            None => Expr::Unary {
                op: "#",
                value: Box::new(Expr::place(Place::name(base))),
            },
        }
    }

    fn elements(base: &str) -> Self::Expr {
        Expr::place(Place::name(base))
    }

    fn binary(op: &'static str, lhs: Self::Expr, rhs: Self::Expr) -> Self::Expr {
        Expr::Binary {
            op: match op {
                "&&" => "and",
                "||" => "or",
                "!=" => "~=",
                other => other,
            },
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        }
    }

    fn unary(op: &'static str, value: Self::Expr) -> Self::Expr {
        Expr::Unary {
            op: match op {
                "!" => "not",
                other => other,
            },
            value: Box::new(value),
        }
    }

    fn paren(value: Self::Expr) -> Self::Expr {
        Expr::Paren(Box::new(value))
    }

    /// The identity on every integer — Lua's integer holds every IDL
    /// width's pattern and `runtime/lua` range-checks at the octet —
    /// and a rewrite on two things that are not integers:
    ///
    /// - a **boolean** discriminator holds `true`/`false`, and
    ///   `v._d == 1` is never true against a boolean, so an integer
    ///   label becomes the literal;
    /// - **`signed char`**, C's reading of a `map`'s `char` key, which
    ///   this column computes as `((cp + 128) % 256) - 128` — Lua's `%`
    ///   is floored, so the two's-complement wrap is exact.
    ///
    /// A `char` label needs nothing: the member holds the code point as
    /// an integer already.
    fn cast(type_: Self::Type, value: Self::Expr) -> Self::Expr {
        if type_.0 == SIGNED_CHAR {
            return Expr::Paren(Box::new(Expr::Binary {
                op: "-",
                lhs: Box::new(Expr::Paren(Box::new(Expr::Binary {
                    op: "%",
                    lhs: Box::new(Expr::Paren(Box::new(Expr::Binary {
                        op: "+",
                        lhs: Box::new(value),
                        rhs: Box::new(Expr::int(128)),
                    }))),
                    rhs: Box::new(Expr::int(256)),
                }))),
                rhs: Box::new(Expr::int(128)),
            }));
        }
        if type_.0 == "boolean" {
            if let Expr::Literal(Literal::Int(text)) = &value {
                if text.chars().all(|c| c.is_ascii_digit() || c == '-') && !text.is_empty() {
                    return Expr::bool(text != "0");
                }
            }
        }
        value
    }

    /// The value, unchanged: Lua passes every table by reference and
    /// takes no address.
    fn handoff(value: Self::Expr, _form: Handoff) -> Self::Expr {
        value
    }

    // -- statements ----------------------------------------------------

    /// `local name = value`, or `local name`. The type is a tag Lua has
    /// no syntax for and is dropped.
    fn declare(_type_: Self::Type, name: &str, value: Option<Self::Expr>) -> Self::Stmt {
        Stat::Local {
            doc: Doc::none(),
            name: Ident::new(name),
            value,
        }
    }

    /// An assignment where the walk built one, the call statement
    /// everywhere else — the only expression Lua's grammar takes as a
    /// statement, and the only other thing the walk performs.
    fn perform(value: Self::Expr) -> Self::Stmt {
        if let Expr::Binary { op: "=", lhs, rhs } = value {
            return Stat::Assign {
                doc: Doc::none(),
                target: Self::place_of(*lhs),
                value: *rhs,
            };
        }
        Stat::Perform(value)
    }

    /// `return v`: this column's decoders answer with the sample —
    /// there is no out-parameter to fill — so the walk's bare
    /// `give_back(None)` returns the local the body built.
    fn give_back(value: Option<Self::Expr>) -> Self::Stmt {
        Stat::Return(Some(value.unwrap_or_else(|| Expr::place(Place::name("v")))))
    }

    fn when(
        condition: Self::Expr,
        then: Self::Block,
        otherwise: Option<Self::Block>,
    ) -> Self::Stmt {
        Stat::If {
            condition,
            then,
            otherwise: otherwise.filter(|body| !body.is_empty()),
        }
    }

    /// `for counter = 0, bound - 1 do … end`. The counter stays
    /// 0-based — [`Self::index`] carries the whole 1-based convention —
    /// and Lua's inclusive upper bound takes the minus one.
    fn count_up(counter: &str, bound: Self::Expr, body: Self::Block) -> Self::Stmt {
        Stat::NumericFor {
            counter: Ident::new(counter),
            from: Expr::int(0),
            to: Expr::Binary {
                op: "-",
                lhs: Box::new(bound),
                rhs: Box::new(Expr::int(1)),
            },
            body,
        }
    }

    fn until(condition: Self::Expr, body: Self::Block) -> Self::Stmt {
        Stat::While { condition, body }
    }

    fn leave() -> Self::Stmt {
        Stat::Break
    }

    fn note(doc: Doc) -> Self::Stmt {
        Stat::Comment(doc)
    }

    /// The call, alone: Lua warns about nothing and `(nil) call` is not
    /// a statement.
    fn drop_value(value: Self::Expr) -> Self::Stmt {
        Stat::Perform(value)
    }

    /// Nothing: an unread parameter is not a diagnostic Lua has, and a
    /// bare name is not a statement.
    fn mark_used(_name: &str) -> Vec<Self::Stmt> {
        Vec::new()
    }

    /// `place = ZD.zd_dec_T(r, zd_tc)`: decoders answer with the value
    /// and the destination is the assignment's left-hand side.
    fn read_call(
        callee: &str,
        place: Self::Expr,
        _form: Handoff,
        behaviour: Self::Expr,
    ) -> Self::Stmt {
        Stat::Assign {
            doc: Doc::none(),
            target: Self::place_of(place),
            value: Self::codec_call(callee, vec![Self::name("r"), behaviour]),
        }
    }

    /// `v = ZD.zd_dec_T(r, zd_tc)` — a `typedef`, handed straight on.
    fn read_forward(callee: &str) -> Self::Stmt {
        Self::read_call(
            callee,
            Expr::place(Place::name("v")),
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

    /// Room for `count` values: a fresh table, and — for a `map`, whose
    /// entries are written half at a time — one `{}` per slot ahead of
    /// the fill, because `v[i].key = …` needs the slot table to exist.
    /// A sequence's element decoder assigns the slot outright and needs
    /// only the outer table; Lua tables grow as they are written.
    fn make_room(
        base: &str,
        _element: Self::Type,
        count: Self::Expr,
        assigned_whole: bool,
        fill: Vec<Self::Stmt>,
    ) -> Vec<Self::Stmt> {
        let mut out = vec![Stat::Assign {
            doc: Doc::none(),
            target: Self::place_from_name(base),
            value: Expr::Table(lua::Table::default()),
        }];
        if !assigned_whole {
            out.push(Stat::NumericFor {
                counter: Ident::new("zd_e"),
                from: Expr::int(0),
                to: Expr::Binary {
                    op: "-",
                    lhs: Box::new(count),
                    rhs: Box::new(Expr::int(1)),
                },
                body: vec![Stat::Assign {
                    doc: Doc::none(),
                    target: Place::Index {
                        base: Box::new(Place::name(base)),
                        key: Box::new(Expr::Binary {
                            op: "+",
                            lhs: Box::new(Expr::place(Place::name("zd_e"))),
                            rhs: Box::new(Expr::int(1)),
                        }),
                    },
                    value: Expr::Table(lua::Table::default()),
                }],
            });
        }
        out.extend(fill);
        out
    }

    /// Drop everything past `count`: shorten from the border, one
    /// element at a time — `t[#t] = nil` moves the border down — until
    /// the bound holds. §7.3.1.2.1.12.3's TRIM.
    fn keep_first(base: &str, count: Self::Expr) -> Vec<Self::Stmt> {
        vec![Stat::While {
            condition: Expr::Binary {
                op: ">",
                lhs: Box::new(Expr::Unary {
                    op: "#",
                    value: Box::new(Expr::place(Place::name(base))),
                }),
                rhs: Box::new(count),
            },
            body: vec![Stat::Assign {
                doc: Doc::none(),
                target: Place::Index {
                    base: Box::new(Place::name(base)),
                    key: Box::new(Expr::Unary {
                        op: "#",
                        value: Box::new(Expr::place(Place::name(base))),
                    }),
                },
                value: Expr::Nil,
            }],
        }]
    }

    /// The subscript, with nothing before or after it: a Lua table
    /// element is an ordinary place at every element type.
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

    /// `v.field ~= nil` — in Lua the absence of a value is the absence
    /// of the key, so presence is the key being there.
    fn present(base: &str, field: &str) -> Self::Expr {
        Expr::Binary {
            op: "~=",
            lhs: Box::new(Self::member(base, field)),
            rhs: Box::new(Expr::Nil),
        }
    }

    /// Nothing on the way in — writing the member is what makes it
    /// present — and `v.field = nil` on the way out, which removes the
    /// key.
    fn set_present(base: &str, field: &str, present: bool) -> Vec<Self::Stmt> {
        if present {
            return Vec::new();
        }
        vec![Stat::Assign {
            doc: Doc::none(),
            target: Place::name(base).field(field),
            value: Expr::Nil,
        }]
    }

    /// `v.field = nil` for every case: a union's branches are keys beside
    /// `_d` on one table, and a branch the discriminator does not select
    /// is a key that is not there. The same statement as an absent
    /// `@optional` member, [`Self::set_present`] with `false`.
    fn clear_union_cases(base: &str, fields: &[String]) -> Vec<Self::Stmt> {
        fields
            .iter()
            .flat_map(|field| Self::set_present(base, field, false))
            .collect()
    }

    /// The member, with nothing to do first: there is no wrapper to
    /// engage before a table key can be written.
    fn optional_destination(base: &str, field: &str) -> (Vec<Self::Stmt>, Self::Expr) {
        (Vec::new(), Self::member(base, field))
    }

    /// The member itself: the key holds the value or is absent.
    fn optional_value(_ast: &Ast, _id: Id<TypeNode>, base: &str, field: &str) -> Self::Expr {
        Self::member(base, field)
    }

    /// `v.field` — a union's branches sit beside `_d` on one table.
    fn case_member(base: &str, field: &str) -> Self::Expr {
        Self::member(base, field)
    }

    /// `entry.key` and `entry.value` — a map entry is a table with the
    /// two keys `adapter/lua.rs` names.
    fn entry_half(entry: Self::Expr, key: bool) -> Self::Expr {
        Self::field_of(entry, if key { "key" } else { "value" })
    }

    fn absent() -> Self::Expr {
        Expr::Nil
    }

    /// Nothing: a Lua string owns its storage and a read that ran out
    /// of memory raised long before this question.
    fn materialised(_value: Self::Expr) -> Option<Self::Expr> {
        None
    }

    /// A local the body writes into: for the one type this is asked
    /// for — the map entry marker — a fresh table, so `zd_scratch.key`
    /// has somewhere to land.
    fn storage(type_: Self::Type, name: &str) -> Self::Stmt {
        if type_.0 == MAP_ENTRY {
            return Stat::Local {
                doc: Doc::none(),
                name: Ident::new(name),
                value: Some(Expr::Table(lua::Table::default())),
            };
        }
        Stat::Local {
            doc: Doc::none(),
            name: Ident::new(name),
            value: None,
        }
    }

    /// `function ZD.zd_enc_T(w, v) … end`, or the reading form that
    /// opens from Table 9 and returns the sample. A forward declaration
    /// is answered with nothing: `ZD.<name>` resolves at the call, so a
    /// recursive or mutually recursive type needs none.
    fn routine(spec: Routine<Self>) -> Option<Self::Decl> {
        let body = spec.body?;
        let mut params = vec![Ident::new(if spec.writing { "w" } else { "r" })];
        if spec.writing {
            params.push(Ident::new("v"));
        }
        for (_, name) in &spec.extra {
            params.push(Ident::new(name));
        }
        let body = if spec.writing {
            body
        } else {
            Self::reading_body(&spec.name, body)
        };
        Some(Stat::Function(lua::Function {
            doc: spec.doc,
            target: Place::name(CODECS).field(&spec.name),
            params,
            body,
        }))
    }

    // -- the materialiser's boundary ----------------------------------

    /// # Errors
    /// Never; the signature matches the emitters around it.
    fn default_into(
        ast: &Ast,
        id: Id<TypeNode>,
        place: Self::Expr,
    ) -> Result<Self::Stmt, EncodeError> {
        Ok(crate::default_lua::default_into(ast, id, place))
    }

    /// # Errors
    /// As [`crate::default_lua::member_default`]: a `@default` this
    /// column cannot spell as a Lua expression is refused rather than
    /// silently replaced by Table 9's zero.
    fn member_default(
        ast: &Ast,
        node: &MemberNode,
        type_: Id<TypeNode>,
        place: Self::Expr,
    ) -> Result<Vec<Self::Stmt>, EncodeError> {
        crate::default_lua::member_default(ast, node, type_, place)
    }

    /// `M.a.b.Colour.values.RED` — the declared value out of the
    /// descriptor the materialiser wrote, through its own path
    /// derivation, so the codec cannot part from it over a spelling.
    /// An enumeration member is an integer here, so the constant
    /// compares directly against what the member holds.
    fn enumerator_constant(ast: &Ast, id: Id<TypeNode>, enumerator: &str) -> Self::Expr {
        match declared_place(ast, id) {
            Ok(place) => Expr::place(place.field("values").field(enumerator)),
            Err(_) => Self::name(enumerator),
        }
    }
}

/// The Lua holder of a value, as a tag.
///
/// Only `boolean` does work — [`Lua::cast`] rewrites a union label
/// against it — and the rest are carried for the reader of the
/// generated tree. The mapping is `adapter/lua.rs`'s.
///
/// zerodds-lint: recursion-depth 64 (collection nesting; bounded by `MAX_TYPE_DEPTH`)
fn value_type_of(ast: &Ast, id: Id<TypeNode>, depth: usize) -> LuaType {
    use zerodds_ast::PrimitiveKind;
    if depth > MAX_TYPE_DEPTH {
        return LuaType("table".to_string());
    }
    let Ok(node) = ast.type_(id) else {
        return LuaType("table".to_string());
    };
    LuaType(
        match &node.kind {
            TypeKind::Primitive(kind) => match kind {
                PrimitiveKind::Boolean => "boolean",
                PrimitiveKind::Float32 | PrimitiveKind::Float64 => "number",
                // The decimal text.
                PrimitiveKind::Float128 => "string",
                // A char is the code point; every other width is an
                // integer too.
                _ => "integer",
            },
            TypeKind::String { .. } | TypeKind::Fixed { .. } => "string",
            TypeKind::Enumeration { .. } | TypeKind::Bitmask { .. } => "integer",
            TypeKind::Alias { aliased } => {
                return aliased.resolved().map_or_else(
                    || LuaType("table".to_string()),
                    |target| value_type_of(ast, target, depth + 1),
                );
            }
            TypeKind::ValueBox { boxed } => {
                return boxed.resolved().map_or_else(
                    || LuaType("table".to_string()),
                    |target| value_type_of(ast, target, depth + 1),
                );
            }
            _ => "table",
        }
        .to_string(),
    )
}

/// The bound a `string`, a `sequence` or a `map` carries, as the suffix
/// that keeps two of them apart.
///
/// The tag names the routine a hoisted collection's codec is emitted
/// under, and one body is emitted per name. With the bound left out,
/// `sequence<long>` and `sequence<long, 4>` are one name and whichever
/// was declared first decides: measured on
/// `@final struct S { sequence<long> b; sequence<long, 4> a; };`, the
/// generated Lua held no `zd_over` check at all, so `a` accepted a
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

/// The tag a codec function is named after, from the IDL kind and never
/// from the Lua spelling — Lua's spelling is `integer` for ten of the
/// fifteen primitives, and a tag built from it would give them one
/// codec. The same scheme `adapter::python::type_tag` uses, without the
/// escape: Lua renames nothing, and the tag lands behind `ZD.zd_enc_`
/// where no keyword can collide.
///
/// zerodds-lint: recursion-depth 64 (collection nesting; bounded by `MAX_TYPE_DEPTH`)
fn type_tag(ast: &Ast, id: Id<TypeNode>, depth: usize) -> String {
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
        // A **named** typedef keeps a tag of its own, unlike the Python
        // column's, and the difference is measured: `ordered_types_from`
        // can visit an alias ahead of a forward-declared target —
        // `recursive-typedef-sequence` does exactly that — and two
        // types sharing one tag means the alias's forwarding body takes
        // the name and the target's real body is never emitted, so the
        // forwarder called itself until the stack ran out. A distinct
        // tag makes the alias an ordinary one-line forwarder whichever
        // of the two is visited first. An anonymous alias is the
        // frontend's own wrapper for an inline type and stays
        // transparent.
        TypeKind::Alias { aliased } => {
            if !node.name.is_empty() {
                let scoped = crate::adapter::lua::scoped_name(ast, id);
                return scoped.trim_start_matches(':').replace("::", "_");
            }
            match aliased.resolved() {
                Some(target) => type_tag(ast, target, depth + 1),
                None => "unknown".to_string(),
            }
        }
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
        | TypeKind::Native => {
            let scoped = crate::adapter::lua::scoped_name(ast, id);
            scoped
                .trim_start_matches(':')
                .replace("::", "_")
                .replace(['<', '>'], "_")
        }
        TypeKind::Any => "any".to_string(),
        TypeKind::ValueBase => "valuebase".to_string(),
        TypeKind::ObjectRef { .. } => "objectref".to_string(),
        TypeKind::AnnotationType { .. } => "annotation".to_string(),
    }
}
