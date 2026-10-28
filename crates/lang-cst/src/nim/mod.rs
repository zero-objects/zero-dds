// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Nim concrete syntax tree. Spec §1.
//!
//! Nim's declaration syntax is a *section*: `type`, `const` and `var`
//! each open a block, and every declaration under it is one line plus an
//! indented body. So the top of this tree is [`File`] holding
//! [`Section`]s and not a flat list of declarations, which is the one
//! structural difference from `crate::go` — there, `type` is written
//! once per declaration and the order of the file is the order of the
//! declarations.
//!
//! Three things this tree holds that the sibling trees do not, and each
//! is Nim's:
//!
//! 1. **An export marker per declaration.** [`TypeDecl::exported`],
//!    [`Field::exported`] and [`ConstDecl::exported`] render as a `*`
//!    after the name. Nim's manual (Declaration and Scopes, "Export
//!    marker") makes a declared symbol reachable from another module if
//!    and only if it carries one. That is what
//!    [`crate::common::Visibility::Public`] means here — and unlike the
//!    Go column, where visibility is the case of the first letter, it
//!    renames nothing: the IDL spelling of every declaration survives
//!    into the generated file.
//! 2. **No export marker on an enumeration's fields.** [`EnumField`] has
//!    no `exported` at all. The `enumDecl` production of Nim's grammar
//!    takes a `symbol` where the object and constant productions take an
//!    `identVis` — there is no place in the syntax for the `*` — and the
//!    fields of an exported enumeration are reachable from another
//!    module anyway. A tree that carried the flag would let a rule ask
//!    for output the grammar has no form for.
//! 3. **[`Type::Option`] as a form of its own.** `Option[T]` is
//!    `std/options`, not a built-in, so a file using it has to import
//!    the module. Keeping it apart from [`Type::Named`] is what lets
//!    `zerodds_lang_rules::adapter::nim` decide the import from the
//!    tree rather than from a flag it carried alongside.
//!
//! ## Identifiers are the hazard of this column
//!
//! Nim compares identifiers **after normalisation**: the first character
//! keeps its case, the rest is lower-cased and every underscore is
//! removed (Nim manual, Lexical Analysis, "Identifiers & Keywords";
//! `std/strutils.nimIdentNormalize` is the same rule as a function). So
//! `fooBar`, `foo_bar` and `fooBAR` are one identifier and `FooBar` is a
//! different one. Two IDL members spelled `foo_bar` and `fooBar` are
//! legal IDL (IDL 4.2 §7.2.3.1 only forbids a pair differing in case
//! alone) and one Nim field. [`normalise`] computes the key that finds
//! that pair; the materialiser refuses it by name.
//!
//! The same rule bounds what an escape can be. Nim's identifier
//! production is
//!
//! ```text
//! IDENTIFIER ::= letter ( ['_'] (letter | digit) )*
//! ```
//!
//! so an underscore must sit between two symbol characters: a leading
//! one, a trailing one and a doubled one are each a lexical error and
//! not a name. `type_`, which is what the Go and Python columns escape a
//! keyword to, is therefore not a Nim identifier at all, and `_d`, which
//! is what every other column calls a union discriminator, is not one
//! either. [`escape_name`] repairs the underscores and then *strops* —
//! encloses the name in backticks, which is Nim's own syntax for using a
//! keyword as a name.
//!
//! ## What is here beyond the types
//!
//! The statement layer a codec body writes: [`ProcDecl`], [`Stmt`] and
//! the expression forms a generated encoder or decoder needs. Only the
//! forms the shared codec walk emits — Nim's own richer set (`case`,
//! `defer`, templates, generics of our own) is deliberately not here:
//! a form no walk builds is a renderer arm no test reaches. Every form
//! has a renderer test, so no arm of `zerodds_lang_render::nim` is code
//! that only claims to work.
//!
//! One Nim rule shapes two of the statements. An expression statement
//! whose value is not `void` does not compile — Nim's "expression has no
//! type (or is ambiguous)" / discard check — so a value deliberately
//! dropped is [`Stmt::Discard`], and [`Stmt::Expr`] is only ever handed
//! a call that answers nothing.

use crate::common::{BodyText, Doc, Ident, Literal};
use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// A Nim source file.
///
/// One file per IDL file. Nim's unit of scope is the module and a module
/// *is* a file — there is no syntax for nesting one inside another — so
/// nested IDL modules cannot nest here. They flatten onto the declared
/// name, which is what `zerodds_lang_rules::adapter::nim` does and what
/// the C and Go columns do for the same reason.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct File {
    /// The leading comment, written above everything else.
    pub doc: Doc,
    /// `import` lines, in written order.
    pub imports: Vec<Import>,
    /// The sections, in written order.
    pub sections: Vec<Section>,
}

/// One `import` line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Import {
    /// The module path, written as Nim writes it: `std/options`.
    pub path: String,
}

impl Import {
    /// An import of `path`.
    #[must_use]
    pub fn new(path: &str) -> Self {
        Self {
            path: path.to_string(),
        }
    }
}

/// One top-level section.
///
/// A section and not a declaration, because that is the unit Nim's
/// syntax has: `type` is written once and every declaration under it is
/// indented beneath it. It is also what makes the order inside a `type`
/// section not matter — see [`Section::Type`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Section {
    /// `type` and the declarations under it.
    ///
    /// Every type an IDL file declares goes in **one** of these.
    /// Declarations in a single `type` section may name each other in
    /// any order, including mutually (Nim manual, "Type sections"), so
    /// unlike `zerodds_lang_rules::adapter::c` there is no post-order
    /// walk of the type graph and unlike the OCaml column there is no
    /// `and` to thread between the declarations that recurse. The
    /// generated file tracks the declaration order of the source.
    Type(Vec<TypeDecl>),
    /// `const` and the declarations under it.
    Const(Vec<ConstDecl>),
    /// A comment standing on its own.
    Comment(Doc),
    /// `@verbatim` text at file level, copied into the output unchanged —
    /// OMG IDL 4.2 §8.3.5.1 asks for the text "verbatim", so raw lines and
    /// never a comment.
    Verbatim(String),
}

/// One declaration inside a `type` section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeDecl {
    /// The comment written above it.
    pub doc: Doc,
    /// The name declared.
    pub name: Ident,
    /// Whether the `*` export marker is written after the name.
    pub exported: bool,
    /// What it declares.
    pub body: TypeBody,
    /// `@verbatim` text attached to this declaration.
    pub text: TypeText,
}

/// `@verbatim` text attached to one declaration of a `type` section.
///
/// A section is one list of declarations — Nim resolves the names inside
/// it as a group — so the text OMG IDL 4.2 §8.3.5.1 places before and
/// after a declaration rides on the declaration it surrounds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TypeText {
    /// Lines before the declaration, at the section's indentation.
    pub before: Vec<String>,
    /// Lines at the start and the end of an object's body.
    pub body: BodyText,
    /// Lines after the declaration, at the section's indentation.
    pub after: Vec<String>,
}

/// The right-hand side of a type declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeBody {
    /// `= object`, and the fields under it.
    ///
    /// An empty field list is legal and renders as a bare `= object`,
    /// which Nim accepts: a struct with no members needs no refusal.
    Object(Vec<Field>),
    /// `= enum`, and the fields under it.
    Enum(Vec<EnumField>),
    /// `= T` — a plain alias.
    ///
    /// Transparent, never `distinct`: IDL 4.2 §7.4.1.4.3 says a typedef
    /// "does not define a new type", and a `distinct` one would need an
    /// explicit conversion at every use.
    Alias(Type),
}

/// One field of an object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// The comment written above it.
    pub doc: Doc,
    /// The name, as it will be written.
    pub name: Ident,
    /// Whether the `*` export marker is written after the name.
    ///
    /// A field without one cannot be read from another module, so every
    /// generated field carries it. Nim manual, "Export marker".
    pub exported: bool,
    /// The type.
    pub type_: Type,
}

/// One field of an enumeration.
///
/// No export marker: the `enumDecl` production takes a `symbol`, not an
/// `identVis`, and the fields of an exported enumeration are reachable
/// from another module without one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumField {
    /// The comment written above it.
    pub doc: Doc,
    /// The name.
    pub name: Ident,
    /// The ordinal, when it is written out.
    ///
    /// The materialiser always writes one, because `@value` admits gaps
    /// and a field with no ordinal takes its predecessor's plus one.
    pub value: Option<Expr>,
}

/// One declaration inside a `const` section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstDecl {
    /// The comment written above it.
    pub doc: Doc,
    /// The name declared.
    pub name: Ident,
    /// Whether the `*` export marker is written after the name.
    pub exported: bool,
    /// The declared type, absent where the initialiser gives it.
    pub type_: Option<Type>,
    /// The value.
    pub value: Expr,
}

/// A Nim type, as it will be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    /// A built-in.
    Primitive(Primitive),
    /// A declared name.
    Named(Ident),
    /// `seq[T]`.
    Seq(Box<Type>),
    /// `array[N, T]`.
    ///
    /// `N` is a plain integer, which in Nim means the index type
    /// `0 .. N-1` (Nim manual, "Array and sequence types": an array
    /// whose length is given as an integer is indexed from zero). That
    /// is the same indexing an IDL array has.
    Array {
        /// The length.
        length: Box<Expr>,
        /// The element type.
        element: Box<Type>,
    },
    /// `Option[T]`, from `std/options`.
    Option(Box<Type>),
    /// `ref T` — a traced reference.
    ///
    /// What `@external` becomes (DDS-XTypes 1.3 Table 21): the member is
    /// held outside the containing object. `ref` and not `ptr`, because
    /// a `ref` is traced — the runtime owns the cell and frees it with
    /// the object, where a `ptr` would make every generated type an
    /// ownership question the IDL does not answer.
    ///
    /// It is also the only form in this tree whose size does not depend
    /// on what it points at, which is what makes a self-recursive
    /// declaration finite.
    Ref(Box<Type>),
    /// `proc (a: T, ...): R` — a procedural type.
    Proc {
        /// The parameters, in order.
        params: Vec<Param>,
        /// The result, absent where the procedure returns nothing.
        result: Option<Box<Type>>,
    },
}

impl Type {
    /// A type spelled by one name.
    #[must_use]
    pub fn named(text: &str) -> Self {
        Self::Named(Ident::new(text))
    }

    /// `seq[T]`.
    #[must_use]
    pub fn seq(element: Self) -> Self {
        Self::Seq(Box::new(element))
    }

    /// `Option[T]`.
    #[must_use]
    pub fn option(inner: Self) -> Self {
        Self::Option(Box::new(inner))
    }

    /// `ref T`.
    #[must_use]
    pub fn reference(target: Self) -> Self {
        Self::Ref(Box::new(target))
    }

    /// `array[n, T]`.
    #[must_use]
    pub fn array(length: i128, element: Self) -> Self {
        Self::Array {
            length: Box::new(Expr::Literal(Literal::int(length))),
            element: Box::new(element),
        }
    }

    /// Whether this type, or anything inside it, is an [`Type::Option`].
    ///
    /// What decides whether the file imports `std/options`. Read off the
    /// tree rather than tracked beside it, so a member that reaches an
    /// `Option` through a sequence or an array cannot leave the import
    /// out.
    #[must_use]
    pub fn needs_options(&self) -> bool {
        match self {
            Self::Option(_) => true,
            Self::Seq(inner) | Self::Array { element: inner, .. } | Self::Ref(inner) => {
                inner.needs_options()
            }
            Self::Proc { params, result } => {
                params.iter().any(|param| param.type_.needs_options())
                    || result.as_ref().is_some_and(|type_| type_.needs_options())
            }
            Self::Primitive(_) | Self::Named(_) => false,
        }
    }
}

/// One parameter of a procedural type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    /// The name.
    pub name: Ident,
    /// The type.
    pub type_: Type,
}

/// A Nim built-in type.
///
/// `int` and `uint` are **not** here and that is the point: their width
/// is the platform's — "the same size as a pointer" (Nim manual,
/// "Pre-defined integer types") — so a member declared with one would
/// have a different size on a 32-bit target than on a 64-bit one, for a
/// wire format that fixes every width. Nim's `float` is absent for the
/// same class of reason: it is an alias for `float64` and naming it
/// would hide which of the two an IDL `double` asked for.
///
/// `char` is absent although Nim has one. Nim's `char` is exactly one
/// byte, which is the width an IDL `char` has, but it is a distinct
/// ordinal type with no arithmetic: `c + 1` does not compile, and a
/// codec that has to add an offset to an octet would need a conversion
/// at every use. An IDL `char` is held in a `uint8` instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Primitive {
    /// `bool`
    Bool,
    /// `int8`
    Int8,
    /// `int16`
    Int16,
    /// `int32`
    Int32,
    /// `int64`
    Int64,
    /// `uint8`
    UInt8,
    /// `uint16`
    UInt16,
    /// `uint32`
    UInt32,
    /// `uint64`
    UInt64,
    /// `float32`
    Float32,
    /// `float64`
    Float64,
    /// `string`
    String,
    /// `pointer` — an untyped, untraced address.
    ///
    /// What a `native` declaration takes when no facet named a mapping.
    /// Untraced on purpose: Nim's `ref` is garbage-collected and would
    /// make the runtime own memory an IDL `native` says nothing about.
    Pointer,
}

impl Primitive {
    /// Every built-in the tree can hold.
    pub const ALL: [Self; 13] = [
        Self::Bool,
        Self::Int8,
        Self::Int16,
        Self::Int32,
        Self::Int64,
        Self::UInt8,
        Self::UInt16,
        Self::UInt32,
        Self::UInt64,
        Self::Float32,
        Self::Float64,
        Self::String,
        Self::Pointer,
    ];

    /// How it is written.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::Int8 => "int8",
            Self::Int16 => "int16",
            Self::Int32 => "int32",
            Self::Int64 => "int64",
            Self::UInt8 => "uint8",
            Self::UInt16 => "uint16",
            Self::UInt32 => "uint32",
            Self::UInt64 => "uint64",
            Self::Float32 => "float32",
            Self::Float64 => "float64",
            Self::String => "string",
            Self::Pointer => "pointer",
        }
    }
}

/// An expression.
///
/// Only the forms a constant declaration, an enumeration ordinal and an
/// array length take. [`Expr::Binary`] is here because a bitmask
/// constant is `1 shl 4`, and [`Expr::Convert`] because that shift has
/// to happen in the bitmask's own unsigned width.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    /// A literal.
    Literal(Literal),
    /// A name.
    Name(Ident),
    /// `a op b`.
    Binary {
        /// The operator, as written. Nim spells the shifts `shl`/`shr`
        /// and the bitwise operators `and`/`or`/`xor`.
        op: &'static str,
        /// The left operand.
        lhs: Box<Expr>,
        /// The right operand.
        rhs: Box<Expr>,
    },
    /// `T(v)` — a conversion.
    ///
    /// Range-checked at run time, which is what a value crossing into a
    /// narrower type should be; a reinterpretation is [`Expr::Cast`].
    Convert {
        /// The target type.
        type_: Box<Type>,
        /// The value.
        value: Box<Expr>,
    },
    /// `(a)` — parentheses written on purpose.
    ///
    /// The tree carries no operator precedence, so a builder that needs
    /// a bracket says so.
    Paren(Box<Expr>),
    /// `f(a, b)` — a call. The callee is an expression because a method
    /// call is a call on a dotted access.
    Call {
        /// What is called.
        func: Box<Expr>,
        /// The arguments, in order.
        args: Vec<Expr>,
    },
    /// `value.name` — a field access, a method name, or a property.
    Dot {
        /// The value accessed.
        value: Box<Expr>,
        /// The name accessed.
        name: Ident,
    },
    /// `a[i]`.
    Index {
        /// The value indexed.
        value: Box<Expr>,
        /// The index.
        index: Box<Expr>,
    },
    /// `op a` — a prefix operator, written with a space: `not a`.
    Unary {
        /// The operator, in Nim's spelling (`not`).
        op: &'static str,
        /// The operand.
        value: Box<Expr>,
    },
    /// `cast[T](v)` — a bit reinterpretation.
    ///
    /// Separate from [`Expr::Convert`] because the two answer different
    /// questions: `int8(200'u8)` raises `RangeDefect` where
    /// `cast[int8](200'u8)` is the two's complement `-56` — which is
    /// what a signed reading of an unsigned holder needs.
    Cast {
        /// The target type.
        type_: Box<Type>,
        /// The value.
        value: Box<Expr>,
    },
    /// `newSeq[T](n)` — storage for `n` elements, every slot
    /// default-initialised.
    NewSeq {
        /// The element type.
        element: Box<Type>,
        /// The length.
        length: Box<Expr>,
    },
    /// `a[]` — the value behind a `ref`.
    Deref(Box<Expr>),
}

impl Expr {
    /// An integer literal.
    #[must_use]
    pub fn int(value: i128) -> Self {
        Self::Literal(Literal::int(value))
    }

    /// A bare name.
    #[must_use]
    pub fn name(text: &str) -> Self {
        Self::Name(Ident::new(text))
    }

    /// `value.name`.
    #[must_use]
    pub fn dot(value: Self, name: &str) -> Self {
        Self::Dot {
            value: Box::new(value),
            name: Ident::new(name),
        }
    }

    /// `func(args)`.
    #[must_use]
    pub fn call(func: Self, args: Vec<Self>) -> Self {
        Self::Call {
            func: Box::new(func),
            args,
        }
    }
}

/// A statement, as a codec body writes one.
///
/// Only the forms the shared codec walk emits. Nim's indentation is the
/// block structure, so a statement carries its sub-statements as lists
/// and the renderer owns the depth.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    /// `var name: T = value`, `var name: T`, or `var name = value`.
    Var {
        /// The name declared.
        name: Ident,
        /// The declared type; absent asks the initialiser to carry it,
        /// and then a value is required.
        type_: Option<Type>,
        /// The initialiser.
        value: Option<Expr>,
    },
    /// `place = value`.
    Assign {
        /// The place written.
        place: Expr,
        /// The value.
        value: Expr,
    },
    /// An expression evaluated for its effect.
    ///
    /// Only for a call that answers nothing: Nim refuses an expression
    /// statement with a value. A value dropped on purpose is
    /// [`Stmt::Discard`].
    Expr(Expr),
    /// `discard value` — a value dropped, and said so.
    Discard(Expr),
    /// `return`, with or without a value.
    Return(Option<Expr>),
    /// `if condition: .. else: ..`.
    If {
        /// The condition.
        condition: Expr,
        /// The then-branch.
        then: Vec<Stmt>,
        /// The else-branch, absent where there is none.
        otherwise: Option<Vec<Stmt>>,
    },
    /// `for counter in 0'u32 ..< bound: ..`.
    ///
    /// The counter is a `uint32`: every bound the shared walk hands over
    /// is a count off the wire (already `uint32`) or an integer literal,
    /// which Nim converts against the range's type.
    For {
        /// The counter's name.
        counter: Ident,
        /// The exclusive upper bound.
        bound: Box<Expr>,
        /// The body.
        body: Vec<Stmt>,
    },
    /// `while condition: ..`.
    While {
        /// The condition.
        condition: Expr,
        /// The body.
        body: Vec<Stmt>,
    },
    /// `break`.
    Break,
    /// A comment on its own line.
    Comment(Doc),
}

/// `proc name(params): R = body`, or the forward declaration without
/// the `=`.
///
/// Nim resolves a module-scope name only after its declaration, so a
/// pair of mutually recursive codecs needs the bodiless form ahead of
/// the bodies — the same reason the C column emits prototypes, and
/// unlike Go and D, where any order compiles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcDecl {
    /// The comment written above it.
    pub doc: Doc,
    /// The name declared.
    pub name: Ident,
    /// Whether the name carries Nim's export marker, `*`.
    ///
    /// Nim has no `public` keyword: a symbol is visible outside its
    /// module if and only if its declaration writes a `*` after the
    /// name (Nim manual, "Export marker"). Without it a generated codec
    /// is reachable only from the file it is in, which is why this is a
    /// field and not a rendering detail — a caller in another module is
    /// exactly what a generated codec is for.
    pub exported: bool,
    /// The parameters, in order.
    pub params: Vec<Param>,
    /// The result type, absent where the procedure answers nothing.
    pub returns: Option<Type>,
    /// The body; `None` renders the forward declaration.
    ///
    /// An empty body renders as `discard`, which is Nim's spelling of a
    /// procedure that does nothing — a bare `=` with nothing indented
    /// under it does not parse.
    pub body: Option<Vec<Stmt>>,
}

/// The key Nim compares two identifiers by.
///
/// The first character keeps its case; every other character is
/// lower-cased and every underscore is dropped. Nim manual, Lexical
/// Analysis, "Identifiers & Keywords" — `std/strutils.nimIdentNormalize`
/// is the same rule written as a procedure.
///
/// This is the column's central hazard. `foo_bar` and `fooBar` are two
/// IDL members — IDL 4.2 §7.2.3.1 forbids only a pair that differs in
/// case alone, and these differ by an underscore — and one Nim field.
/// The materialiser runs every scope's names through this and refuses a
/// collision by name, rather than emitting an object whose second field
/// silently replaces the first.
///
/// It is also why the reserved check below is done on the normalised
/// form: Nim's lexer looks a symbol up in the keyword table by the same
/// normalised key, so `t_ype` is the keyword `type` and not an
/// identifier that resembles it.
#[must_use]
pub fn normalise(name: &str) -> String {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut out = String::new();
    out.push(first);
    for ch in chars {
        if ch == '_' {
            continue;
        }
        for lower in ch.to_lowercase() {
            out.push(lower);
        }
    }
    out
}

/// The name an IDL identifier takes in Nim.
///
/// Two steps, in this order, and the order is load-bearing.
///
/// 1. **Repair the underscores.** Nim's identifier production is
///    `IDENTIFIER ::= letter ( ['_'] (letter | digit) )*` (Nim manual,
///    Lexical Analysis), so an underscore is only a character of a name
///    when a letter or a digit follows it and something precedes it. A
///    leading underscore, a trailing one and a doubled one are each a
///    lexical error — the compiler's own message for the second is
///    `invalid token: trailing underscore`. IDL admits all three
///    (IDL 4.2 §7.2.3 gives the identifier as
///    `[A-Za-z_][A-Za-z0-9_]*`), so `_d`, `count_` and `a__b` all arrive
///    here. They are repaired rather than refused, because the repair
///    changes nothing Nim can observe: Nim's own comparison drops
///    interior underscores anyway, so `a__b`, `a_b` and `ab` were
///    already one identifier. A name left with nothing a Nim identifier
///    may start with takes the `zd` prefix.
/// 2. **Strop what is a keyword.** Backticks, which is Nim's syntax for
///    using a keyword as a name (`` `type` ``, `` `end` ``). A trailing
///    underscore, which is what the Go and Python columns append, is not
///    an option here for the reason step 1 gives — `type_` does not
///    lex — and a prefix or a suffix that did lex would rename the
///    member, which the export marker otherwise makes unnecessary in
///    this column.
///
/// The check is on [`normalise`]d text because that is the key Nim's
/// lexer uses: `t_ype` reaches the keyword table entry for `type`.
/// Stropping a name that did not strictly need it costs nothing —
/// `` `payload` `` and `payload` are the same symbol — so the check
/// erring wide is safe in the direction it errs.
#[must_use]
pub fn escape_name(name: &str) -> String {
    let repaired = repair(name);
    if RESERVED.contains(&normalise(&repaired).as_str()) {
        let mut out = String::from("`");
        out.push_str(&repaired);
        out.push('`');
        return out;
    }
    repaired
}

/// The prefix a name with no legal first character takes.
///
/// `zd` and not an underscore, because an underscore is exactly what
/// cannot start a Nim identifier.
const REPAIR_PREFIX: &str = "zd";

/// `name`, with the underscores Nim's identifier production does not
/// admit removed.
///
/// Leading underscores are dropped, trailing ones are dropped and a run
/// of them collapses to one. See [`escape_name`] for why this is a
/// repair rather than a refusal.
fn repair(name: &str) -> String {
    let mut out = String::new();
    let mut pending_underscore = false;
    for ch in name.chars() {
        if ch == '_' {
            // Only recorded, never written: it is written when the next
            // character arrives, so a trailing run reaches the end of
            // the loop unwritten and a doubled one is written once.
            pending_underscore = !out.is_empty();
            continue;
        }
        if pending_underscore {
            out.push('_');
            pending_underscore = false;
        }
        out.push(ch);
    }
    // Nim's `letter` is `'A'..'Z' | 'a'..'z' | '\x80'..'\xff'`; an IDL
    // identifier is ASCII (IDL 4.2 §7.2.3), so a name that does not
    // start with an ASCII letter after the repair cannot start a Nim
    // identifier at all.
    if !out
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic())
    {
        let mut prefixed = String::from(REPAIR_PREFIX);
        prefixed.push_str(&out);
        return prefixed;
    }
    out
}

/// Every Nim keyword.
///
/// The list in the Nim manual's Lexical Analysis section, in the order
/// it gives them. `end`, `interface` and `using` are reserved and unused
/// by the language, and they are here because reserved is reserved: a
/// declaration named `end` does not parse whether or not the compiler
/// has a use for the word.
///
/// Written here rather than in a materialiser so that a second backend
/// over this tree cannot spell the set differently and produce a file
/// that disagrees about which names are safe.
pub const RESERVED: &[&str] = &[
    "addr",
    "and",
    "as",
    "asm",
    "bind",
    "block",
    "break",
    "case",
    "cast",
    "concept",
    "const",
    "continue",
    "converter",
    "defer",
    "discard",
    "distinct",
    "div",
    "do",
    "elif",
    "else",
    "end",
    "enum",
    "except",
    "export",
    "finally",
    "for",
    "from",
    "func",
    "if",
    "import",
    "in",
    "include",
    "interface",
    "is",
    "isnot",
    "iterator",
    "let",
    "macro",
    "method",
    "mixin",
    "mod",
    "nil",
    "not",
    "notin",
    "object",
    "of",
    "or",
    "out",
    "proc",
    "ptr",
    "raise",
    "ref",
    "return",
    "shl",
    "shr",
    "static",
    "template",
    "try",
    "tuple",
    "type",
    "using",
    "var",
    "when",
    "while",
    "xor",
    "yield",
];

/// The module-scope names a generated file writes its own types in terms
/// of.
///
/// Not keywords: Nim lets a declaration shadow any of them, and the file
/// parses. It stops compiling at the next use of the original — every
/// field type in a generated file is one of these, so a declaration
/// named `seq` would take the name out from under every `seq[T]` in the
/// same module.
///
/// Stropping does not help here, which is what separates this list from
/// [`RESERVED`]: `` `seq` `` *is* the identifier `seq`, so backticks
/// change nothing. A trailing-underscore escape is not available either
/// (see [`escape_name`]). The materialiser therefore refuses a
/// declaration that reaches one of these by name, rather than renaming
/// it or emitting a file whose failure names a Nim identifier instead of
/// the IDL declaration behind it.
///
/// Matched against [`normalise`]d text, so `s_eq` is caught and `Seq` is
/// not — the first character's case is significant in Nim, and `Seq` is
/// a different identifier from `seq`.
pub const PREDECLARED: &[&str] = &[
    // The integer and floating-point types every member is written in.
    "bool", "int8", "int16", "int32", "int64", "uint8", "uint16", "uint32", "uint64", "float32",
    "float64", // The three built-ins a member's type is built from.
    "string", "seq", "array",
    // What a `native` declaration with no facet mapping becomes.
    "pointer", // `std/options`, and the type an `@optional` member takes from it.
    "options", "Option",
];

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
    fn no_primitive_has_an_implementation_defined_width() {
        // Nim's `int` and `uint` are "the same size as a pointer" (Nim
        // manual, "Pre-defined integer types"), and `float` is an alias
        // for `float64` that hides which of the two was asked for.
        for primitive in Primitive::ALL {
            assert!(
                !matches!(primitive.as_str(), "int" | "uint" | "float"),
                "{} has no fixed width, or hides the one it has",
                primitive.as_str()
            );
        }
    }

    #[test]
    fn normalisation_keeps_the_first_character_and_folds_the_rest() {
        // Nim manual, Lexical Analysis: the first character's case is
        // significant, the rest is compared case- and
        // underscore-insensitively.
        assert_eq!(normalise("fooBar"), "foobar");
        assert_eq!(normalise("foo_bar"), "foobar");
        assert_eq!(normalise("fooBAR"), "foobar");
        assert_eq!(normalise("FooBar"), "Foobar");
        assert_eq!(normalise(""), "");
    }

    #[test]
    fn two_idl_spellings_that_nim_cannot_tell_apart_share_a_key() {
        // The pair IDL 4.2 §7.2.3.1 permits and Nim does not
        // distinguish. This is the whole reason `normalise` is public.
        assert_eq!(normalise("foo_bar"), normalise("fooBar"));
        // And the pair Nim does distinguish, because the first
        // character's case is significant.
        assert_ne!(normalise("fooBar"), normalise("FooBar"));
    }

    #[test]
    fn a_keyword_is_stropped_because_a_trailing_underscore_does_not_lex() {
        // `type_` is not a Nim identifier: the production is
        // `letter ( ['_'] (letter | digit) )*`, so an underscore has to
        // be followed by a letter or a digit.
        assert_eq!(escape_name("type"), "`type`");
        assert_eq!(escape_name("end"), "`end`");
        assert_eq!(escape_name("proc"), "`proc`");
        assert_eq!(escape_name("payload"), "payload");
    }

    #[test]
    fn a_name_that_normalises_onto_a_keyword_is_stropped_too() {
        // Nim's lexer looks a symbol up in the keyword table by the
        // normalised key, so `t_ype` and `tYPE` are both the keyword
        // `type` and would not parse where a name belongs. The backticks
        // hold the spelling the IDL wrote; Nim resolves it to the one
        // symbol either way.
        assert_eq!(escape_name("t_ype"), "`t_ype`");
        assert_eq!(escape_name("tYPE"), "`tYPE`");
        // The first character's case is significant, so this one is not
        // a keyword at all.
        assert_eq!(escape_name("Type"), "Type");
    }

    #[test]
    fn a_leading_underscore_is_removed_because_nim_has_no_name_for_it() {
        // `_d` is the discriminator's name in every other column. Nim's
        // lexer reports `invalid token: _` for it: the identifier
        // production starts at a letter.
        assert_eq!(escape_name("_d"), "d");
        assert_eq!(escape_name("_reserved"), "reserved");
        assert_eq!(escape_name("__x"), "x");
    }

    #[test]
    fn a_trailing_or_doubled_underscore_is_removed_for_the_same_reason() {
        assert_eq!(escape_name("count_"), "count");
        assert_eq!(escape_name("a__b"), "a_b");
        assert_eq!(escape_name("a_b"), "a_b");
        // The repair is invisible to Nim: it compares these as one
        // identifier whatever the underscores do.
        assert_eq!(normalise("a__b"), normalise("a_b"));
    }

    #[test]
    fn a_name_with_nothing_a_nim_identifier_may_start_with_takes_a_prefix() {
        // `_` and `_1` are legal IDL identifiers and neither has a first
        // character Nim admits once the underscore is gone.
        assert_eq!(escape_name("_"), "zd");
        assert_eq!(escape_name("_1a"), "zd1a");
    }

    #[test]
    fn the_reserved_list_has_no_duplicates_and_is_already_normalised() {
        let mut seen: Vec<&str> = Vec::new();
        for name in RESERVED {
            assert!(!seen.contains(name), "{name} is listed twice");
            assert_eq!(normalise(name), *name, "{name} is not in normal form");
            seen.push(name);
        }
    }

    #[test]
    fn the_predeclared_list_has_no_duplicates_and_no_keywords() {
        // A name that is both would be checked twice and refused where
        // stropping would have done.
        let mut seen: Vec<&str> = Vec::new();
        for name in PREDECLARED {
            assert!(!seen.contains(name), "{name} is listed twice");
            assert!(!RESERVED.contains(name), "{name} is a keyword");
            seen.push(name);
        }
    }

    #[test]
    fn an_array_type_nests_outermost_dimension_first() {
        // `long grid[2][3]` is two rows of three, which in Nim is
        // `array[2, array[3, int32]]`. Reversing it silently transposes
        // the data.
        let type_ = Type::array(2, Type::array(3, Type::Primitive(Primitive::Int32)));
        let Type::Array { length, element } = &type_ else {
            panic!("not an array");
        };
        assert_eq!(**length, Expr::int(2));
        let Type::Array { length, .. } = &**element else {
            panic!("the element is not an array");
        };
        assert_eq!(**length, Expr::int(3));
    }

    #[test]
    fn an_option_anywhere_inside_a_type_asks_for_the_import() {
        // The import is read off the tree, so a member that reaches an
        // `Option` through a sequence cannot leave `std/options` out.
        assert!(Type::option(Type::Primitive(Primitive::Int32)).needs_options());
        assert!(Type::seq(Type::option(Type::Primitive(Primitive::Bool))).needs_options());
        assert!(!Type::seq(Type::Primitive(Primitive::Bool)).needs_options());
        // `@optional @external` is an `Option` outside a `ref`, so the
        // import has to survive the reference too.
        assert!(Type::option(Type::reference(Type::named("Node"))).needs_options());
        assert!(!Type::reference(Type::named("Node")).needs_options());
        assert!(
            Type::Proc {
                params: Vec::new(),
                result: Some(Box::new(Type::option(Type::Primitive(Primitive::Int32)))),
            }
            .needs_options()
        );
    }

    #[test]
    fn an_enumeration_field_has_no_export_marker_to_set() {
        // The `enumDecl` production takes a `symbol` where the object
        // and constant productions take an `identVis`, so there is no
        // place in the syntax for a `*`. A tree that carried the flag
        // would let a rule ask for output the grammar has no form for.
        let field = EnumField {
            doc: Doc::none(),
            name: Ident::new("IDLE"),
            value: Some(Expr::int(0)),
        };
        assert_eq!(field.name.as_str(), "IDLE");
    }
}
