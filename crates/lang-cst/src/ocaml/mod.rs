// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The OCaml concrete syntax tree. Spec §1.
//!
//! Two properties of OCaml shape this tree, and neither has a
//! counterpart in the C, Go or Java columns.
//!
//! 1. **A type declaration is a group, not a declaration.** OCaml
//!    resolves names in the order a structure writes them (OCaml manual,
//!    ch. "Module expressions"), and a type that mentions itself or a
//!    sibling has to be written as one `type a = … and b = …` group. So
//!    the top-level node is [`TypeGroup`] and not a single declaration:
//!    a tree that could only hold one type per `type` keyword could not
//!    express `struct Node { sequence<Node> children; };` at all.
//! 2. **The case of the first letter is grammar, not convention.** A
//!    type constructor and a record field are `lowercase-ident`, a
//!    variant constructor and a module name are `capitalized-ident`
//!    (OCaml manual, ch. "Lexical conventions"). Go's
//!    `zerodds_lang_cst::go::exported` moves one letter for visibility
//!    and a file with it in the wrong case still compiles; here the file
//!    does not parse. [`lower_ident`] and [`upper_ident`] are where that
//!    is done, once.
//!
//! ## What is not here
//!
//! No expressions beyond what a `let` binding and a variant conversion
//! need, no functors, no first-class modules, no polymorphic variants,
//! no type parameters. This tree carries the *types* an IDL file becomes,
//! the constants it declares, the two conversions an enumeration needs
//! and the method signatures an `interface` declares.
//!
//! There is no `module` node. The adapter flattens a scoped IDL name
//! onto the declared name, and `zerodds_lang_rules::adapter::ocaml`
//! carries the reason: an `and`-joined recursive group cannot cross a
//! `module … struct … end` boundary, and IDL puts no such restriction on
//! where a cycle may close.

use crate::common::{BodyText, Doc, Ident, Literal};
use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// An OCaml implementation file, `.ml`.
///
/// One file per IDL file. There is no interface file (`.mli`): an
/// `.mli` that omitted a declaration would hide it, and every generated
/// declaration is meant to be reachable. Its absence also keeps the
/// compiler's unused-declaration warnings quiet, because a structure
/// with no signature exports everything.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct File {
    /// The leading comment.
    pub doc: Doc,
    /// Declarations, in written order. The order is the file's meaning:
    /// a name is in scope only after the declaration that binds it.
    pub decls: Vec<Decl>,
}

/// One top-level declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decl {
    /// `type a = … and b = …` — one mutually recursive group.
    Types(TypeGroup),
    /// `let name = expr`, optionally annotated.
    Let(LetDecl),
    /// `class type name = object … end`.
    ClassType(ClassTypeDecl),
    /// A comment standing on its own.
    Comment(Doc),
    /// `@verbatim` text, copied into the output unchanged — OMG IDL 4.2
    /// §8.3.5.1 asks for the text "verbatim", so raw lines and never a
    /// comment.
    Verbatim(String),
}

/// A `type` group: one or more declarations joined by `and`.
///
/// Every type in the group may name every other one. A group of one is
/// the ordinary case and renders as a plain `type name = …`; a group of
/// more than one is what a cycle in the IDL type graph becomes, and
/// there is no other way to write it — OCaml resolves a type name only
/// against what precedes it or against the group it is in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeGroup {
    /// Documentation, written above the `type` keyword.
    pub doc: Doc,
    /// The declarations, in written order. Never empty in a rendered
    /// file: `type` with nothing after it does not parse.
    pub items: Vec<TypeDecl>,
}

impl TypeGroup {
    /// A group holding one declaration.
    #[must_use]
    pub fn one(item: TypeDecl) -> Self {
        Self {
            doc: Doc::none(),
            items: alloc::vec![item],
        }
    }

    /// Whether this group is a recursive one, which is what more than
    /// one member means.
    #[must_use]
    pub fn is_recursive_group(&self) -> bool {
        self.items.len() > 1
    }
}

/// One `type name = body` within a group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeDecl {
    /// Documentation.
    pub doc: Doc,
    /// The name declared. A `lowercase-ident`; [`lower_ident`] is what
    /// makes it one.
    pub name: Ident,
    /// What it declares.
    pub body: TypeBody,
    /// `@verbatim` text at the start and the end of a record's body.
    /// Empty for any other body.
    pub body_text: BodyText,
}

/// The right-hand side of a type declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeBody {
    /// `type t = u` — an abbreviation, not a new type.
    ///
    /// OCaml abbreviations are transparent, which is what an IDL
    /// `typedef` says (IDL 4.2 §7.4.1.4.3: an alias "does not define a
    /// new type"). An abbreviation may not be part of an unguarded
    /// cycle; `zerodds_lang_rules::adapter::ocaml` refuses one rather
    /// than emitting a group the compiler rejects.
    Alias(Type),
    /// `type t = { a : u; b : v; }` — a record.
    ///
    /// Never empty: `type t = { }` is a syntax error, and the adapter
    /// writes [`TypeBody::Alias`] of `unit` instead.
    Record(Vec<Field>),
    /// `type t = A | B` — a variant.
    Variant(Vec<Constructor>),
    /// `type t` — abstract, with no values at all.
    Abstract,
}

/// One field of a record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// Documentation.
    pub doc: Doc,
    /// The field's name, a `lowercase-ident`.
    pub name: Ident,
    /// Its type.
    pub type_: Type,
}

/// One constructor of a variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Constructor {
    /// Documentation.
    pub doc: Doc,
    /// The constructor's name, a `capitalized-ident`.
    pub name: Ident,
    /// Its arguments, empty for a constant constructor. Every
    /// constructor this column emits is constant: an IDL enumerator
    /// carries no payload.
    pub args: Vec<Type>,
}

/// `let name : type = body`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LetDecl {
    /// Documentation.
    pub doc: Doc,
    /// The bound name, a `lowercase-ident`.
    pub name: Ident,
    /// The annotation, where one is written.
    ///
    /// Written wherever the declared type is known, so that a constant
    /// of a declared type is checked against it at compile time rather
    /// than inferred into whatever its literal happens to be.
    pub type_: Option<Type>,
    /// The bound expression.
    pub body: Expr,
}

/// `class type name = object … end`.
///
/// A class type and not a record of closures. Both are expressible; the
/// class type is the one whose members are *methods*, so an IDL
/// operation keeps its name and its arity in the type rather than
/// becoming a field holding a function. It also carries no values, which
/// is what an interface declaration is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassTypeDecl {
    /// Documentation.
    pub doc: Doc,
    /// The name, a `lowercase-ident`.
    pub name: Ident,
    /// The methods, in written order. May be empty: `object end` parses.
    pub methods: Vec<Method>,
}

/// One method of a class type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Method {
    /// Documentation.
    pub doc: Doc,
    /// The name, a `lowercase-ident`.
    pub name: Ident,
    /// The parameter types, in order. Empty for an operation with none,
    /// which renders as `method m : result` and takes `unit` nowhere —
    /// a method with no arguments is a value of its result type.
    pub params: Vec<Type>,
    /// The result type. `unit` for an operation returning `void`.
    pub result: Type,
}

/// An OCaml type expression, as it will be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    /// A built-in.
    Primitive(Primitive),
    /// A declared name.
    Named(Ident),
    /// `t array`.
    ///
    /// An array and never a list. A list has no O(1) length, and a
    /// `sequence<T>` is written to the wire as a count followed by the
    /// elements (DDS-XTypes 1.3 §7.4.3.5.3), so a codec over a list
    /// would walk the whole structure once to write its first field.
    Array(Box<Type>),
    /// `t option`.
    Option(Box<Type>),
    /// `(a * b)` — a tuple.
    Tuple(Vec<Type>),
    /// `Obj.t`, `Int32.t` — a name qualified by a module.
    Qualified {
        /// The module path, outermost first.
        module: Vec<Ident>,
        /// The type's own name.
        name: Ident,
    },
}

impl Type {
    /// A type spelled by one name.
    #[must_use]
    pub fn named(text: &str) -> Self {
        Self::Named(Ident::new(text))
    }

    /// `t array`.
    #[must_use]
    pub fn array(element: Self) -> Self {
        Self::Array(Box::new(element))
    }

    /// `t option`.
    #[must_use]
    pub fn option(inner: Self) -> Self {
        Self::Option(Box::new(inner))
    }

    /// `M.t`.
    #[must_use]
    pub fn qualified(module: &str, name: &str) -> Self {
        Self::Qualified {
            module: alloc::vec![Ident::new(module)],
            name: Ident::new(name),
        }
    }
}

/// An OCaml built-in type.
///
/// `nativeint` is deliberately absent: it is the platform's word width
/// (OCaml manual, "Nativeint"), which is 32 bits on one target and 64 on
/// another, for a wire format that fixes every width.
///
/// There is no unsigned type here because OCaml has none — not in the
/// language and not in the standard library. What that costs is decided
/// in `zerodds_lang_rules::adapter::ocaml`, which is where the mapping
/// from an IDL `unsigned long long` is written down and argued.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Primitive {
    /// `bool`
    Bool,
    /// `int` — 63 bits on a 64-bit target and 31 on a 32-bit one
    /// (OCaml manual, "Integers"). Used only where the IDL type fits in
    /// 31 bits, so the difference between the two cannot be reached.
    Int,
    /// `int32`
    Int32,
    /// `int64`
    Int64,
    /// `float` — an IEEE 754 binary64, which is a C `double`.
    Float,
    /// `string`
    String,
    /// `char` — exactly one octet, with no arithmetic of its own.
    Char,
    /// `unit` — one value, carrying nothing.
    Unit,
}

impl Primitive {
    /// Every built-in the tree can hold.
    pub const ALL: [Self; 8] = [
        Self::Bool,
        Self::Int,
        Self::Int32,
        Self::Int64,
        Self::Float,
        Self::String,
        Self::Char,
        Self::Unit,
    ];

    /// How it is written.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::Int => "int",
            Self::Int32 => "int32",
            Self::Int64 => "int64",
            Self::Float => "float",
            Self::String => "string",
            Self::Char => "char",
            Self::Unit => "unit",
        }
    }
}

/// An expression.
///
/// Only the forms a `let` binding takes: a literal, a name, a
/// constructor, an application, and the `function` a variant conversion
/// is. Nothing here can hold a statement, because an OCaml structure has
/// none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    /// A literal, already carrying whatever suffix its type needs —
    /// see [`int32_literal`] and [`int64_literal`].
    Literal(Literal),
    /// A bound name.
    Name(Ident),
    /// A variant constructor used as a value: `IDLE`.
    Construct(Ident),
    /// `f a b`.
    Apply {
        /// The function.
        func: Box<Expr>,
        /// The arguments, in order.
        args: Vec<Expr>,
    },
    /// `function p -> e | p -> e`.
    Function(Vec<Arm>),
    /// `(e)` — parentheses written on purpose.
    ///
    /// The tree carries no operator precedence, so a builder that needs
    /// a bracket says so. An OCaml application binds tighter than
    /// anything, and a negative literal as an argument needs one.
    Paren(Box<Expr>),
}

impl Expr {
    /// A name.
    #[must_use]
    pub fn name(text: &str) -> Self {
        Self::Name(Ident::new(text))
    }

    /// A constructor.
    #[must_use]
    pub fn construct(text: &str) -> Self {
        Self::Construct(Ident::new(text))
    }

    /// `f a`.
    #[must_use]
    pub fn apply(func: Self, args: Vec<Self>) -> Self {
        Self::Apply {
            func: Box::new(func),
            args,
        }
    }
}

/// One `pattern -> expression` of a `function`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arm {
    /// The pattern matched.
    pub pattern: Pattern,
    /// What it evaluates to.
    pub body: Expr,
}

/// A pattern, in the forms a conversion needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pattern {
    /// A literal, matched exactly.
    Literal(Literal),
    /// A constructor with no arguments.
    Construct(Ident),
    /// A name, binding whatever arrives.
    Bind(Ident),
}

/// An `int32` literal, which is a decimal with an `l` suffix.
///
/// The suffix is not decoration: `0` is an `int` and `0l` an `int32`,
/// and OCaml has no implicit conversion between them (manual, "Integer
/// literals"). A generated `let x : int32 = 0` does not type-check.
#[must_use]
pub fn int32_literal(value: i128) -> Literal {
    Literal::Int(alloc::format!("{value}l"))
}

/// An `int64` literal, which is a decimal with an `L` suffix.
#[must_use]
pub fn int64_literal(value: i128) -> Literal {
    Literal::Int(alloc::format!("{value}L"))
}

/// The `lowercase-ident` an IDL identifier becomes.
///
/// Three things happen, and each is forced by the grammar or by a
/// collision the grammar would otherwise hide:
///
/// 1. **The whole name is folded to lower case.** A type constructor, a
///    record field, a value name and a method name are all
///    `lowercase-ident` (OCaml manual, "Lexical conventions"), so
///    `struct Pose` cannot be `type Pose`. Folding the whole name and
///    not only the first letter is what makes `bitmask Flags { READY }`
///    produce `let ready` rather than `let rEADY`. The fold is injective
///    within one IDL scope: IDL 4.2 §7.2.3.1 puts every identifier in one
///    namespace and compares case-insensitively, so `Pose` and `pose` are
///    already a collision the frontend reports. Across scopes it is the
///    flattening of the scoped name that can collide, and
///    `zerodds_lang_rules::adapter::ocaml` checks every declared name in
///    the file against every other.
/// 2. **A name that cannot start a `lowercase-ident` is prefixed.** The
///    grammar admits `_` as a first character, so `_d` needs nothing;
///    `_` alone is the wildcard pattern and not an identifier, and a
///    name starting with a digit is not one either. Both take `zd_`.
/// 3. **A keyword or a name the generated file relies on takes a
///    trailing underscore.** [`RESERVED`] is the list and says which is
///    which.
///
/// The IDL spelling is not lost when any of this fires: the adapter
/// writes it into the comment above the declaration.
#[must_use]
pub fn lower_ident(name: &str) -> String {
    let folded: String = name.chars().flat_map(char::to_lowercase).collect();
    let mut out = match folded.chars().next() {
        None => "zd_".to_string(),
        // `_` on its own is the wildcard pattern; `_d` is a perfectly
        // ordinary identifier and is left alone, because it is the name
        // every other column gives a union's discriminator.
        Some('_') if folded.len() == 1 => "zd_".to_string(),
        Some(first) if first == '_' || first.is_ascii_lowercase() => folded,
        _ => {
            let mut prefixed = String::from("zd_");
            prefixed.push_str(&folded);
            prefixed
        }
    };
    if RESERVED.contains(&out.as_str()) {
        out.push('_');
    }
    out
}

/// The `capitalized-ident` an IDL identifier becomes.
///
/// A variant constructor must start with an upper-case letter, and —
/// unlike a `lowercase-ident` — may not start with an underscore at all
/// (OCaml manual, "Lexical conventions"). So the first letter is
/// upper-cased and a name that begins with anything else takes a `Zd`
/// prefix, which turns the escaped IDL identifier `_value` into
/// `Zd_value`.
///
/// The rest of the name keeps its IDL spelling. An enumerator is
/// conventionally `ALL_CAPS` in IDL and stays that way here, so
/// `enum Mode { IDLE }` gives `type mode = IDLE` and a reader sees the
/// name the source wrote.
#[must_use]
pub fn upper_ident(name: &str) -> String {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return "Zd".to_string();
    };
    if first.is_ascii_uppercase() {
        return name.to_string();
    }
    if first.is_ascii_lowercase() {
        let mut out = String::new();
        for upper in first.to_uppercase() {
            out.push(upper);
        }
        out.push_str(chars.as_str());
        return out;
    }
    let mut out = String::from("Zd");
    out.push_str(name);
    out
}

/// Every name a `lowercase-ident` has to move out of the way of.
///
/// Two groups, and the second is the one a reader will not expect:
///
/// - **The keywords**, as the OCaml manual lists them under "Lexical
///   conventions". A declaration named one of these does not parse.
/// - **The predefined type constructors and the two boolean
///   constructors' lower-case neighbours.** `int`, `string`, `array`,
///   `option` and the rest are ordinary names in OCaml and a structure
///   may shadow them — and a generated file cannot afford to, because
///   every field type in it is one of these. `type option = { … }`
///   parses, and the next member declared `int32 option` then means
///   something else entirely.
///
/// `bool`, `float` and `char` are in the first sense predefined types
/// too, and are listed for the same reason.
pub const RESERVED: &[&str] = &[
    // Keywords, OCaml manual "Lexical conventions".
    "and",
    "as",
    "assert",
    "asr",
    "begin",
    "class",
    "constraint",
    "do",
    "done",
    "downto",
    "else",
    "end",
    "exception",
    "external",
    "false",
    "for",
    "fun",
    "function",
    "functor",
    "if",
    "in",
    "include",
    "inherit",
    "initializer",
    "land",
    "lazy",
    "let",
    "lor",
    "lsl",
    "lsr",
    "lxor",
    "match",
    "method",
    "mod",
    "module",
    "mutable",
    "new",
    "nonrec",
    "object",
    "of",
    "open",
    "or",
    "private",
    "rec",
    "sig",
    "struct",
    "then",
    "to",
    "true",
    "try",
    "type",
    "val",
    "virtual",
    "when",
    "while",
    "with",
    // Predefined type constructors. Shadowing any of these compiles and
    // then changes the meaning of every later declaration.
    "array",
    "bool",
    "bytes",
    "char",
    "exn",
    "float",
    "format",
    "int",
    "int32",
    "int64",
    "lazy_t",
    "list",
    "nativeint",
    "option",
    "ref",
    "result",
    "string",
    "unit",
];

// -- the codec layer ------------------------------------------------------
//
// What a generated codec body is made of. Separate from the declaration
// layer above because the two answer different questions: a declaration
// is what an IDL file *is* in OCaml, a codec body is what a generated
// program *does* — and OCaml has no statements, so every node here is a
// shape the renderer knows how to write as a `unit` expression under
// `;` sequencing. Mutable state is spelled with `ref` cells and array
// stores; an early return and a loop exit are the two exceptions the
// runtime declares (`Zd_return`, `Zd_break`), because OCaml has neither
// `return` nor `break`.
//
// Every expression carries enough for the renderer to type it against
// the memory-shape table in `zerodds_lang_rules::adapter::ocaml`: OCaml
// has no implicit integer conversions at all, so where C writes a cast
// and Java a coercion, this column has to *know* whether a value is an
// `int`, an `int32`, an `int64` or an enumeration variant before it can
// write the conversion function between them. [`ValueKind`] is that
// knowledge, and the renderer's environment carries it per name.

/// The written type of the runtime's pattern holder, `type u64 = int64`
/// in `runtime/ocaml/zdwire.ml`.
///
/// The walk asks for one conversion to an unsigned 64-bit holder and one
/// to a signed one, and OCaml spells both `int64`, so the two would be
/// the same [`CodecType`] and the renderer could not tell them apart.
/// They need different code: a bitmask of bound 32 is held in an
/// `int32`, bit 31 set makes it negative, and `Int64.of_int32` would
/// carry the sign into bits 32..63 — past the bound the runtime then
/// range-checks against. A transparent alias costs nothing at run time
/// and gives the two casts different text.
pub const UNSIGNED_HOLDER: &str = "Zdwire.u64";

/// The OCaml type a codec value has, as far as conversions care.
///
/// Not the full type: an `int option array` and a record are both
/// [`ValueKind::Aggregate`], because no numeric conversion ever reaches
/// them. What is distinguished is exactly what needs a different
/// conversion function or literal suffix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueKind {
    /// An OCaml `int`: every IDL type of at most 16 bits, `char` and
    /// `wchar` as code points, `unsigned long` narrowed at the call,
    /// and every counter, position and length.
    Int,
    /// An `int32`: an IDL `long`.
    Int32,
    /// An `int64`: an IDL `unsigned long` or `long long` member, an
    /// enumeration holder, a bit pattern.
    Int64,
    /// A `float` (binary64): both IDL float widths.
    Float,
    /// A `bool`.
    Bool,
    /// A `string`: IDL strings, and the decimal holders — `unsigned
    /// long long`, `long double`, `fixed`.
    Text,
    /// `unit`.
    UnitValue,
    /// An enumeration variant, by its flattened OCaml type name; the
    /// conversions beside it are `int32_of_<name>` and
    /// `<name>_of_int32`.
    Enum(String),
    /// An array, with its element's kind: what an index reaches.
    ArrayOf(Box<ValueKind>),
    /// An `option`, with what `Some` holds: what `Option.get` reaches.
    OptionOf(Box<ValueKind>),
    /// A record, with each field's kind under its escaped OCaml name:
    /// what a member access reaches. Depth-limited by the builder — a
    /// recursive type's remote fields degrade to [`ValueKind::Aggregate`],
    /// which only costs a conversion nothing reaches that deep.
    RecordOf(Vec<(String, ValueKind)>),
    /// Anything else no conversion reaches, and the deep tail of a
    /// recursive type.
    Aggregate,
    /// A value of one of the runtime's own types, by the runtime's
    /// name for it: "writer", "reader", "dheader", "emheader", "pid",
    /// "try_construct", "status", "role", "reader_mark", "pid_read",
    /// "emheader_read".
    Handle(&'static str),
}

/// A codec-side type: its written OCaml text, its [`ValueKind`], and
/// the expression that builds its Table 9-shaped placeholder — what a
/// `ref` cell or an `Array.make` slot starts as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodecType {
    /// The OCaml type expression, as written in an annotation.
    pub text: String,
    /// What conversions see.
    pub kind: ValueKind,
    /// An expression producing a value of this type. For a scalar it is
    /// the literal zero in the right suffix; for a declared type it is
    /// a call to the generated Table 9 defaulter; for a runtime handle
    /// it is the runtime's inert value.
    pub default: String,
}

/// An expression of a codec body.
///
/// Deliberately close to the shared walk's own vocabulary: the walk
/// hands these shapes over one call at a time and the renderer is where
/// OCaml's spelling — `!x` for a ref read, `Int64.add` for a 64-bit
/// `+`, `{ !v with f = e }` for a member store — is decided, because
/// only the renderer holds the typing environment.
#[derive(Debug, Clone, PartialEq)]
pub enum CodeExpr {
    /// A bound name; the renderer decides whether it reads as `x` or
    /// `!x` from its environment.
    Name(String),
    /// An integer literal with no width of its own; the renderer spells
    /// it in the width of whatever it meets.
    Int(i128),
    /// An integer literal that is an `int64` wherever it appears.
    Int64Lit(i64),
    /// `true` / `false`.
    BoolLit(bool),
    /// A string literal, escaped by the renderer.
    TextLit(String),
    /// A float literal, in the exact spelling given.
    FloatLit(String),
    /// An operation on the runtime's writer or reader:
    /// `Zdwire.<method> <receiver> <args>`.
    Runtime {
        /// `"w"` or `"r"`.
        receiver: &'static str,
        /// The operation's bare name, the shared walk's spelling.
        method: String,
        /// The arguments after the receiver.
        args: Vec<CodeExpr>,
    },
    /// A free function of the runtime: `Zdwire.<method> <args>`.
    Static {
        /// The function's bare name.
        method: String,
        /// Its arguments; empty renders as `()`.
        args: Vec<CodeExpr>,
    },
    /// A call to a generated function — a codec, a defaulter.
    Call {
        /// The generated function's name.
        callee: String,
        /// Its arguments; empty renders as `()`.
        args: Vec<CodeExpr>,
    },
    /// A member of the sample: `v.f`, or `(!v).f` where `v` is the
    /// reader's cell. As a store target it is a functional record
    /// update. `case` marks a union branch, whose field is an `option`:
    /// read as `Option.get`, stored as `Some`.
    Member {
        /// The local holding the record, `v` in every body so far.
        base: String,
        /// The field, already escaped.
        field: String,
        /// Whether this is a union case member.
        case: bool,
    },
    /// The marked destination of an `@optional` member: stores wrap the
    /// value in `Some`.
    OptionSlot {
        /// The local holding the record.
        base: String,
        /// The field, already escaped.
        field: String,
    },
    /// `Option.get` around an `@optional` member's value.
    OptionGet(Box<CodeExpr>),
    /// `<member> <> None`.
    IsSome {
        /// The local holding the record.
        base: String,
        /// The field, already escaped.
        field: String,
    },
    /// `None`.
    Absent,
    /// The whole sample.
    Whole(String),
    /// `a.(i)`.
    Index {
        /// The array.
        base: Box<CodeExpr>,
        /// The index, an `int`.
        index: Box<CodeExpr>,
    },
    /// One half of a map entry record: `.key` or `.value`.
    EntryHalf {
        /// The entry.
        entry: Box<CodeExpr>,
        /// The key half, or the value half.
        key: bool,
    },
    /// A field of a runtime header, `(e).Zdwire.<field>`; the walk's
    /// field name, mapped to the runtime's spelling by the renderer
    /// from the expression's [`ValueKind::Handle`].
    HeaderField {
        /// The header value.
        value: Box<CodeExpr>,
        /// The walk's field name: "pid", "length", "end", ...
        field: String,
    },
    /// `Array.length` of a collection.
    CountOf(String),
    /// The collection itself; C's `_buffer` has no counterpart here.
    Elements(String),
    /// An enumeration constructor used as a value.
    Construct(String),
    /// A binary operation in the walk's spelling; the renderer picks
    /// the OCaml operator family from the operands' kinds.
    Binary {
        /// `==`, `!=`, `<`, `>`, `<=`, `>=`, `&&`, `||`, `+`, `-`,
        /// `*`, `%`, `&`, `|`, `>>`, `<<`, `=` (assignment, which only
        /// [`Stmt::Assign`] ever receives).
        op: &'static str,
        /// Left.
        lhs: Box<CodeExpr>,
        /// Right.
        rhs: Box<CodeExpr>,
    },
    /// A unary operation: `!` (logical not), `~` (bitwise complement).
    Unary {
        /// The walk's operator.
        op: &'static str,
        /// The operand.
        value: Box<CodeExpr>,
    },
    /// Parentheses written on purpose.
    Paren(Box<CodeExpr>),
    /// A conversion into `target`, resolved by the renderer from the
    /// operand's kind — `Int64.of_int32`, `int32_of_<enum>`, a literal
    /// re-spelled with its suffix, C's `signed char` reading of a code
    /// point.
    Convert {
        /// What the value has to become.
        target: CodecType,
        /// The value.
        value: Box<CodeExpr>,
    },
    /// A runtime constant: a policy string, a `try_construct`, a
    /// status, a role — already spelled, with its kind.
    RuntimeConst {
        /// The full spelling, `Zdwire.Discard`, `"specification"`, ...
        text: String,
        /// Its kind.
        kind: ValueKind,
    },
    /// A prebuilt expression with a known kind — what the Table 9
    /// module hands the walk for an explicit `@default`.
    Raw {
        /// The rendered text, already parenthesised as needed.
        text: String,
        /// Its kind.
        kind: ValueKind,
    },
}

/// One step of a codec body.
///
/// Rendered under `;` sequencing; a [`Stmt::Let`] opens a scope that
/// the rest of its block lives in.
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// `let <name> : <type> ref = ref <value or default> in`.
    Let {
        /// The local's type.
        type_: CodecType,
        /// Its name.
        name: String,
        /// The initial value; the type's default where absent.
        value: Option<CodeExpr>,
    },
    /// The discarded scratch entry of an uncounted map's first pass:
    /// declares nothing, and every store into it renders as `ignore`.
    Scratch {
        /// The scratch local's name.
        name: String,
    },
    /// A store: `x := e`, `a.(i) <- e`, `v := { !v with f = e }` — the
    /// renderer picks from the place's shape.
    Assign {
        /// Where.
        place: CodeExpr,
        /// What.
        value: CodeExpr,
    },
    /// [`Stmt::Assign`] with the destination's type known, so the value
    /// is converted first.
    Store {
        /// Where.
        place: CodeExpr,
        /// The destination's type.
        type_: CodecType,
        /// What.
        value: CodeExpr,
    },
    /// `place <- <callee> r <behaviour>` — one nested decoder run.
    ReadInto {
        /// The decoder.
        callee: String,
        /// Where its answer goes.
        place: CodeExpr,
        /// The try-construct behaviour.
        behaviour: CodeExpr,
    },
    /// An expression evaluated for its effect; `ignore`-wrapped where
    /// it answers a value.
    Perform(CodeExpr),
    /// `ignore e` written on purpose.
    Discard(CodeExpr),
    /// `raise Zdwire.Zd_return` — the decoder's early exit.
    Return,
    /// `if c then begin .. end else begin .. end`.
    When {
        /// The condition.
        condition: CodeExpr,
        /// The then-block.
        then: Vec<Stmt>,
        /// The else-block, if any.
        otherwise: Option<Vec<Stmt>>,
    },
    /// `for <counter> = 0 to <bound> - 1 do .. done`, in a `Zd_break`
    /// harness.
    For {
        /// The counter, an `int` in scope of the body.
        counter: String,
        /// The exclusive bound.
        bound: CodeExpr,
        /// The body.
        body: Vec<Stmt>,
    },
    /// `while c do .. done`, in a `Zd_break` harness.
    While {
        /// The condition.
        condition: CodeExpr,
        /// The body.
        body: Vec<Stmt>,
    },
    /// `raise Zdwire.Zd_break`.
    Leave,
    /// A comment.
    Note(Doc),
    /// `<base> := Array.make <count> <element default>` — the reader
    /// making room for a collection.
    Alloc {
        /// The local holding the collection.
        base: String,
        /// The element type, whose default fills the slots until the
        /// fill loop overwrites them.
        element: CodecType,
        /// How many.
        count: CodeExpr,
    },
    /// `<base> := Array.sub !<base> 0 <count>` — §7.3.1.2.1.12.3's TRIM.
    KeepFirst {
        /// The local holding the collection.
        base: String,
        /// How many to keep.
        count: CodeExpr,
    },
}

/// One generated codec function, on its way into a `let rec … and …`
/// chain: OCaml resolves a name against what precedes it, and the chain
/// is what lets mutually recursive types call each other's codecs.
#[derive(Debug, Clone, PartialEq)]
pub struct CodecFn {
    /// The comment above it.
    pub doc: Doc,
    /// Its name.
    pub name: String,
    /// The sample's type.
    pub sample: CodecType,
    /// Whether this is the reading direction: the sample becomes a
    /// `ref` cell seeded from `defaulter` and answered at the end.
    pub reading: bool,
    /// The Table 9 defaulter a reader seeds `v` from.
    pub defaulter: Option<String>,
    /// Parameters after the fixed ones, in written order.
    pub extra: Vec<(CodecType, String)>,
    /// The body.
    pub body: Vec<Stmt>,
}

/// One generated value function — a Table 9 defaulter: `let rec
/// <name> () : <result> = <body>`, chained with `and` so a recursive
/// aggregate's defaulter can reach its own parts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueFn {
    /// The comment above it.
    pub doc: Doc,
    /// Its name.
    pub name: String,
    /// The result type's text.
    pub result: String,
    /// The body, already rendered: a defaulter is a pure value
    /// expression and needs no environment.
    pub body: String,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn every_primitive_writes_its_own_spelling() {
        let mut seen = Vec::new();
        for primitive in Primitive::ALL {
            let text = primitive.as_str();
            assert!(!seen.contains(&text), "{text} is used twice");
            seen.push(text);
        }
        assert_eq!(seen.len(), Primitive::ALL.len());
    }

    #[test]
    fn no_primitive_has_a_platform_dependent_width_a_member_can_reach() {
        // `nativeint` is 32 bits on one target and 64 on another. `int`
        // is 31 or 63 and is here, but only IDL types that fit in 31
        // bits are mapped onto it, which `adapter::ocaml` states and
        // tests.
        for primitive in Primitive::ALL {
            assert_ne!(primitive.as_str(), "nativeint");
        }
    }

    #[test]
    fn a_type_name_folds_to_lower_case_because_the_grammar_says_so() {
        // `type Pose = …` is not an OCaml type declaration: a type
        // constructor is a `lowercase-ident`.
        assert_eq!(lower_ident("Pose"), "pose");
        assert_eq!(lower_ident("pose"), "pose");
    }

    #[test]
    fn the_whole_name_folds_and_not_only_the_first_letter() {
        // `READY` is what an IDL bitmask bit is usually called. Folding
        // one letter would give `rEADY`, which parses and is unreadable.
        assert_eq!(lower_ident("READY"), "ready");
        assert_eq!(lower_ident("MAX_LEN"), "max_len");
    }

    #[test]
    fn a_keyword_takes_a_trailing_underscore() {
        assert_eq!(lower_ident("type"), "type_");
        assert_eq!(lower_ident("method"), "method_");
        assert_eq!(lower_ident("end"), "end_");
        assert_eq!(lower_ident("payload"), "payload");
    }

    #[test]
    fn a_predefined_type_name_escapes_although_it_would_parse() {
        // OCaml permits the shadowing. A generated file cannot afford
        // it: every field type in the file is one of these, and a
        // `type option = …` earlier in the file changes what
        // `int32 option` means later in it.
        for name in ["int", "string", "array", "option", "int32", "float"] {
            assert_eq!(lower_ident(name), alloc::format!("{name}_"));
        }
    }

    #[test]
    fn a_leading_underscore_is_left_alone_because_the_grammar_admits_it() {
        // `_d` is the discriminator's name in every other column, and a
        // `lowercase-ident` may begin with an underscore.
        assert_eq!(lower_ident("_d"), "_d");
        assert_eq!(lower_ident("_reserved"), "_reserved");
        // `_` on its own is the wildcard pattern and binds nothing.
        assert_eq!(lower_ident("_"), "zd_");
    }

    #[test]
    fn a_constructor_capitalises_and_cannot_start_with_an_underscore() {
        assert_eq!(upper_ident("IDLE"), "IDLE");
        assert_eq!(upper_ident("idle"), "Idle");
        // A `capitalized-ident` has no underscore form at all.
        assert_eq!(upper_ident("_value"), "Zd_value");
        assert_eq!(upper_ident(""), "Zd");
    }

    #[test]
    fn the_reserved_list_has_no_duplicates() {
        let mut seen: Vec<&str> = Vec::new();
        for name in RESERVED {
            assert!(!seen.contains(name), "{name} is listed twice");
            seen.push(name);
        }
    }

    #[test]
    fn an_int32_literal_carries_the_suffix_its_type_needs() {
        // `let x : int32 = 0` does not type-check; `0l` does.
        assert_eq!(int32_literal(0), Literal::Int("0l".to_string()));
        assert_eq!(int32_literal(-4), Literal::Int("-4l".to_string()));
        assert_eq!(int64_literal(7), Literal::Int("7L".to_string()));
    }

    #[test]
    fn a_group_of_one_is_not_a_recursive_group() {
        let group = TypeGroup::one(TypeDecl {
            body_text: crate::common::BodyText::none(),
            doc: Doc::none(),
            name: Ident::new("pose"),
            body: TypeBody::Record(Vec::new()),
        });
        assert!(!group.is_recursive_group());
        assert_eq!(group.items.len(), 1);
    }
}
