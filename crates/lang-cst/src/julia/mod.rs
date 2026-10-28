// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Julia concrete syntax tree. Spec §1.
//!
//! Julia writes a type after the name and a colon — `x::Int32` — and a
//! type expression is one expression that reads left to right, so the
//! tree carries no declarator to split around a name the way
//! `zerodds_lang_render::c` has to. `long grid[2][3]` is a
//! [`Type::NTuple`] holding a [`Type::NTuple`] holding a
//! [`Type::Primitive`], and the renderer writes it in that order.
//!
//! Four things this tree holds that no other column's does, and each is
//! Julia's:
//!
//! 1. **A struct has no mutability field.** There is no
//!    [`StructDecl`] arm for `mutable struct` and that is the decision,
//!    not an omission: an immutable `struct` is Julia's default form,
//!    and a decoded sample is a value rather than a handle a second
//!    reference can change underneath the first. A tree that could spell
//!    `mutable struct` would let a rule choose per declaration, and the
//!    choice would then be in seven places instead of one.
//! 2. **A supertype on a struct, and an abstract type to name.**
//!    [`StructDecl::supertype`] and [`Decl::Abstract`] exist for exactly
//!    one job: a type graph with a cycle in it. Julia evaluates a field's
//!    type expression when the `struct` is defined, so a name declared
//!    later in the file is an `UndefVarError` — measured on Julia 1.12.6,
//!    and measured to be no different inside `Vector{…}` than outside it.
//!    An `abstract type` declared first has no fields and therefore no
//!    forward reference of its own, which is what breaks the cycle.
//! 3. **No visibility field.** Julia's export list is a `module`'s, and
//!    this column writes no `module` (see
//!    `zerodds_lang_rules::adapter::julia`), so
//!    [`crate::common::Visibility`] has no Julia spelling here.
//! 4. **No field annotation of any kind.** Julia has no counterpart of
//!    Go's `reflect.StructTag`: a field carries a name and a type and
//!    nothing else. Where the Go tree puts the IDL spelling in a tag,
//!    this one has only [`Field::doc`] — a comment — and the materialiser
//!    says so where it uses it.
//!
//! ## What is here, and what the second pass added
//!
//! The tree started as the *types* an IDL file becomes and nothing else.
//! The wire codecs are the second pass, and they are what
//! [`FunctionDef`], [`Statement`], [`Expr::FreeCall`], [`Expr::Field`],
//! [`Expr::Index`], [`Expr::Range`], [`Expr::Unary`] and
//! [`Expr::AnyVector`] arrived for:
//! `crates/lang-rules/src/emit/julia.rs` builds every one of them out of
//! the shared walk, and each has a renderer test in
//! `zerodds_lang_render::julia`. The bound holds in the other direction
//! too — a variant with no producer and no test would be code that only
//! claims to work, which is why there is no `do` block, no comprehension
//! and no macro call beyond `@enum`.

use crate::common::{BodyText, Doc, Ident, Literal};
use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// A Julia source file.
///
/// One file per IDL file, and one namespace inside it. Julia's `module`
/// nests properly, and this tree still has no module node: a module body
/// is one contiguous block and a second `module A … end` in the same file
/// replaces the first binding rather than extending it, so declarations
/// ordered by dependency cannot always be grouped by IDL module. The
/// scope goes into the name instead —
/// `zerodds_lang_rules::adapter::julia` carries the measurement.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct File {
    /// The leading comment, written at the top of the file.
    pub doc: Doc,
    /// Declarations, in written order. The order is load-bearing in
    /// Julia in a way it is not in Go: see [`Decl::Abstract`].
    pub decls: Vec<Decl>,
}

/// One top-level declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decl {
    /// `abstract type Name end`.
    ///
    /// Declares a name with no fields, which is the only declaration in
    /// Julia that can precede everything it will later be related to.
    /// A struct in a cycle is declared `Name <: ZdFwd_Name` and the
    /// member that has to name it before it exists names the abstract
    /// type. Also what an IDL `interface` becomes, for a different
    /// reason: Julia has no interface construct, and an abstract type
    /// with a documented method contract is the language's own answer.
    Abstract(AbstractDecl),
    /// `struct Name … end`.
    Struct(StructDecl),
    /// `@enum Name::Base A = 0 …`.
    Enum(EnumDecl),
    /// `const NAME = value`.
    ///
    /// One node for a constant, a typedef and a bitmask's holder: Julia
    /// spells all three `const`, because a type is an ordinary value and
    /// a binding to one is an ordinary binding. A tree that distinguished
    /// them would be recording the rule's intent rather than the syntax.
    Const(ConstDecl),
    /// `function name end` — a generic function with no methods.
    ///
    /// A declaration and not a definition: it binds the name and adds no
    /// method, which is how Julia declares that a function exists before
    /// anything implements it.
    Function(FunctionDecl),
    /// `function name(params)::T … end` — a function with a body.
    ///
    /// What a generated codec is. Separate from [`Decl::Function`]
    /// because the stub has no parameter list at all and merging the two
    /// would let a rule write half of each.
    FunctionDef(FunctionDef),
    /// A comment standing on its own.
    Comment(Doc),
    /// `@verbatim` text, copied into the output unchanged — OMG IDL 4.2
    /// §8.3.5.1 asks for the text "verbatim", so raw lines and never a
    /// comment.
    Verbatim(String),
}

/// `abstract type Name end`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractDecl {
    /// Documentation.
    pub doc: Doc,
    /// The name declared.
    pub name: Ident,
}

/// `struct Name [<: Super] … end`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructDecl {
    /// Documentation.
    pub doc: Doc,
    /// The name declared.
    pub name: Ident,
    /// The supertype, when this struct is named before it is declared.
    ///
    /// Absent for every declaration in a file whose type graph is
    /// acyclic, which is nearly all of them.
    pub supertype: Option<Ident>,
    /// The fields, in declaration order. Empty is legal: a struct with
    /// no fields is a singleton type, which is what an IDL aggregate with
    /// no members is.
    pub fields: Vec<Field>,
    /// `@verbatim` text at the start and the end of the body.
    pub body_text: BodyText,
}

/// One field of a struct.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// The comment written above it.
    ///
    /// The only place a field can carry anything but a name and a type.
    /// Julia has no `reflect.StructTag`, so the IDL spelling of a renamed
    /// member is here and a program cannot read it back at run time —
    /// which is why the generated codec is told the IDL name rather than
    /// asked to recover it.
    pub doc: Doc,
    /// The name, as it will be written.
    pub name: Ident,
    /// The type.
    pub type_: Type,
}

/// `@enum Name::Base begin … end`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumDecl {
    /// Documentation.
    pub doc: Doc,
    /// The name declared.
    pub name: Ident,
    /// The integer type the values are held in, written out.
    ///
    /// `@enum` defaults to `Int32` when no base is given; it is written
    /// anyway, because the default is the macro's and the width here is
    /// the IDL's.
    pub base: Primitive,
    /// The enumerators, in declaration order. Never empty: `@enum` with
    /// no members raises at load time (measured on Julia 1.12.6:
    /// "reducing over an empty collection is not allowed").
    pub members: Vec<Enumerator>,
}

/// One enumerator of an `@enum`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Enumerator {
    /// The comment written above it.
    pub doc: Doc,
    /// The name, which `@enum` binds at module scope.
    pub name: Ident,
    /// The value, always written out. `@enum` numbers from zero when a
    /// value is omitted, and `@value` admits gaps.
    pub value: Expr,
}

/// `const NAME = value`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstDecl {
    /// Documentation.
    pub doc: Doc,
    /// The name bound.
    pub name: Ident,
    /// The value, which may be a type.
    pub value: Expr,
}

/// `function name end`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionDecl {
    /// Documentation, which is where the signature goes: a function
    /// declared this way has no argument list.
    pub doc: Doc,
    /// The name bound.
    pub name: Ident,
}

/// `function name(params)[::T] … end`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionDef {
    /// Documentation.
    pub doc: Doc,
    /// The name bound.
    pub name: Ident,
    /// The parameters, in written order.
    pub params: Vec<Param>,
    /// The return annotation. Absent leaves it unwritten, which in Julia
    /// is not the same as `::Any`: an annotation converts every return
    /// value, and a codec whose returns are already the right type has
    /// nothing to ask a conversion for.
    pub returns: Option<Type>,
    /// The body, in written order. Empty is legal: `function f() end`
    /// defines a method that answers `nothing`.
    pub body: Vec<Statement>,
}

/// One parameter of a [`FunctionDef`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    /// The name.
    pub name: Ident,
    /// The annotation, absent where the parameter carries none.
    pub type_: Option<Type>,
}

/// One statement of a function body.
///
/// Only the forms the shared codec walk builds. Julia's `x = y` is an
/// expression, and it is still a statement node here: an assignment that
/// could nest inside another expression is one the renderer could bury
/// where nobody reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    /// `local name[::T] [= value]`.
    ///
    /// `local` written out, although Julia would bind the name without
    /// it: an annotated local — `local n::UInt32 = read_u32(r)` — is the
    /// one place a generated body states a width, and the keyword is
    /// what makes the annotation attach to the binding rather than parse
    /// as a type assertion on an assignment.
    Local {
        /// The name bound.
        name: Ident,
        /// The annotation, absent where the local carries none.
        type_: Option<Type>,
        /// The initial value, absent for a binding assigned later.
        value: Option<Expr>,
    },
    /// `place = value` — a store into somewhere that already exists.
    Assign {
        /// Where the value goes.
        place: Expr,
        /// The value.
        value: Expr,
    },
    /// An expression evaluated for its effect.
    Expr(Expr),
    /// `if condition … [else …] end`.
    ///
    /// No `elseif`: a chain is an `else` whose body is one more
    /// [`Statement::If`], which is what `elseif` abbreviates and what
    /// the codec walk builds.
    If {
        /// The condition.
        condition: Expr,
        /// The body taken when it holds. Empty is legal Julia.
        then: Vec<Statement>,
        /// The body taken when it does not.
        otherwise: Option<Vec<Statement>>,
    },
    /// `for counter in iter … end`.
    For {
        /// The induction variable.
        counter: Ident,
        /// What is iterated — a [`Expr::Range`] in every producer.
        iter: Expr,
        /// The body.
        body: Vec<Statement>,
    },
    /// `while condition … end`.
    While {
        /// The condition.
        condition: Expr,
        /// The body.
        body: Vec<Statement>,
    },
    /// `break`.
    Break,
    /// `return`, with or without a value.
    Return(Option<Expr>),
    /// A `#` comment on lines of its own.
    Comment(Doc),
}

/// A Julia type, as it will be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    /// A built-in.
    Primitive(Primitive),
    /// A declared name.
    Named(Ident),
    /// `Vector{T}`.
    Vector(Box<Type>),
    /// `NTuple{N, T}`.
    ///
    /// The length is part of the type, which is the one IDL fact Julia's
    /// own type system enforces without a codec being asked.
    NTuple {
        /// The length.
        length: Box<Expr>,
        /// The element type.
        element: Box<Type>,
    },
    /// `Union{A, B}`.
    ///
    /// A real Julia type a method can dispatch on, not a tagged
    /// alternative: `Union{Int32, Nothing}` is what an absent
    /// `@optional` member is held in.
    Union(Vec<Type>),
}

impl Type {
    /// A type spelled by one name.
    #[must_use]
    pub fn named(text: &str) -> Self {
        Self::Named(Ident::new(text))
    }

    /// `Vector{T}`.
    #[must_use]
    pub fn vector(element: Self) -> Self {
        Self::Vector(Box::new(element))
    }

    /// `NTuple{n, T}`.
    #[must_use]
    pub fn ntuple(length: i128, element: Self) -> Self {
        Self::NTuple {
            length: Box::new(Expr::int(length)),
            element: Box::new(element),
        }
    }

    /// `Union{T, Nothing}`.
    ///
    /// In that order, which is the order the manual writes it in and the
    /// order every other column's optional reads in: the type first, its
    /// absence second.
    #[must_use]
    pub fn optional(inner: Self) -> Self {
        Self::Union(alloc::vec![inner, Self::Primitive(Primitive::Nothing)])
    }
}

/// A Julia built-in type.
///
/// `Int` and `UInt` are **not** here and that is the point: `Int` is an
/// alias for `Int64` on a 64-bit platform and `Int32` on a 32-bit one
/// (Julia manual, "Integers and Floating-Point Numbers"), so a member
/// declared with one would have a different width on a Raspberry Pi than
/// on the host that generated it, for a wire format that fixes every
/// width. `BigFloat` is absent for a related reason — its precision is a
/// run-time global set by `setprecision` and not part of the type
/// (measured: `precision(BigFloat)` is 256 by default, and changing it
/// changes every existing value's arithmetic).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Primitive {
    /// `Bool`
    Bool,
    /// `Int8`
    Int8,
    /// `Int16`
    Int16,
    /// `Int32`
    Int32,
    /// `Int64`
    Int64,
    /// `UInt8`
    UInt8,
    /// `UInt16`
    UInt16,
    /// `UInt32`
    UInt32,
    /// `UInt64`
    UInt64,
    /// `Float32`
    Float32,
    /// `Float64`
    Float64,
    /// `String` — UTF-8, and carrying its own length.
    String,
    /// `Nothing`, whose one value is `nothing`.
    Nothing,
    /// `Any` — the top of the type lattice.
    Any,
}

impl Primitive {
    /// Every built-in the tree can hold.
    pub const ALL: [Self; 14] = [
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
        Self::Nothing,
        Self::Any,
    ];

    /// How it is written.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bool => "Bool",
            Self::Int8 => "Int8",
            Self::Int16 => "Int16",
            Self::Int32 => "Int32",
            Self::Int64 => "Int64",
            Self::UInt8 => "UInt8",
            Self::UInt16 => "UInt16",
            Self::UInt32 => "UInt32",
            Self::UInt64 => "UInt64",
            Self::Float32 => "Float32",
            Self::Float64 => "Float64",
            Self::String => "String",
            Self::Nothing => "Nothing",
            Self::Any => "Any",
        }
    }
}

/// An expression.
///
/// Only the forms a constant declaration, an enumerator and an array
/// length take. [`Expr::Binary`] is here because a bitmask constant is
/// `Flags(1) << 4`, and [`Expr::Type`] because a Julia type is an
/// ordinary value: `const Distance = Int32` is a binding whose
/// right-hand side is a type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    /// A literal.
    Literal(Literal),
    /// A name.
    Name(Ident),
    /// A type written where a value belongs.
    Type(Box<Type>),
    /// `T(v)` — a conversion.
    ///
    /// The callee is a [`Type`] and not an arbitrary expression, so the
    /// tree cannot hold a call to something that is not a constructor.
    /// Written rather than a bare number because an IDL constant has a
    /// declared type: `const LIMIT = 8` binds an `Int64` on this host and
    /// an `Int32` on a 32-bit one, and `UInt8(8)` binds what the IDL
    /// said.
    Call {
        /// The type constructed.
        func: Box<Type>,
        /// The arguments, in written order.
        args: Vec<Expr>,
    },
    /// `a op b`.
    Binary {
        /// The operator, as written.
        op: &'static str,
        /// The left operand.
        lhs: Box<Expr>,
        /// The right operand.
        rhs: Box<Expr>,
    },
    /// `(a)` — parentheses written on purpose.
    ///
    /// The tree carries no operator precedence, so a builder that needs a
    /// bracket says so.
    Paren(Box<Expr>),
    /// `func(args…)` — a call to a function by name.
    ///
    /// The other calling form beside [`Expr::Call`], whose callee is a
    /// [`Type`]: a runtime operation — `write_u32(w, x)` — is not a
    /// conversion, and a tree that spelled both through one node would
    /// let a conversion take an argument list no constructor has.
    FreeCall {
        /// The function called. May be dotted (`Base.length`), which is
        /// an [`Ident`]'s business to carry.
        func: Ident,
        /// The arguments, in written order.
        args: Vec<Expr>,
    },
    /// `value.name` — a field of a value.
    Field {
        /// What the field is taken of.
        value: Box<Expr>,
        /// The field.
        name: Ident,
    },
    /// `value[index]`.
    ///
    /// The index is written exactly as given: Julia counts from one, and
    /// which side of that conversion a subscript stands on is the
    /// emitter's decision, not the renderer's.
    Index {
        /// What is subscripted.
        value: Box<Expr>,
        /// The subscript.
        index: Box<Expr>,
    },
    /// `start:stop` — a range, inclusive at both ends.
    Range {
        /// The first value.
        start: Box<Expr>,
        /// The last value.
        stop: Box<Expr>,
    },
    /// `op value` — `!x`, `~x`.
    Unary {
        /// The operator, as written.
        op: &'static str,
        /// The operand.
        value: Box<Expr>,
    },
    /// `Any[a, b]` — a `Vector{Any}` literal.
    ///
    /// `Any` written out, because a bare `[a, b]` infers its element
    /// type from the values: a defaulter answering `[Int32(0)]` would
    /// bind a `Vector{Int32}` the reader then cannot put a `nothing`
    /// into, and the mutable scratch a decoder writes through is
    /// `Vector{Any}` on purpose.
    AnyVector(Vec<Expr>),
}

impl Expr {
    /// An integer literal.
    #[must_use]
    pub fn int(value: i128) -> Self {
        Self::Literal(Literal::int(value))
    }

    /// `T(value)`.
    #[must_use]
    pub fn convert(type_: Type, value: Self) -> Self {
        Self::Call {
            func: Box::new(type_),
            args: alloc::vec![value],
        }
    }

    /// A bare name.
    #[must_use]
    pub fn name(text: &str) -> Self {
        Self::Name(Ident::new(text))
    }

    /// `func(args…)`.
    #[must_use]
    pub fn free_call(func: &str, args: Vec<Self>) -> Self {
        Self::FreeCall {
            func: Ident::new(func),
            args,
        }
    }

    /// `value.name`.
    #[must_use]
    pub fn field(value: Self, name: &str) -> Self {
        Self::Field {
            value: Box::new(value),
            name: Ident::new(name),
        }
    }

    /// `value[index]`, exactly as given.
    #[must_use]
    pub fn index(value: Self, index: Self) -> Self {
        Self::Index {
            value: Box::new(value),
            index: Box::new(index),
        }
    }

    /// `start:stop`.
    #[must_use]
    pub fn range(start: Self, stop: Self) -> Self {
        Self::Range {
            start: Box::new(start),
            stop: Box::new(stop),
        }
    }
}

/// The name an IDL identifier takes at Julia's module scope.
///
/// A trailing underscore where the identifier is a Julia keyword or one
/// of the names a generated file writes.
///
/// Both halves are load-bearing and for different reasons:
///
/// - a **keyword** does not parse as a name at all, so `struct S; end::Int32; end`
///   is not a struct with a field called `end`;
/// - a **name the file itself writes** — `Int32`, `String`, `Vector`,
///   `Nothing` — is shadowed silently. `const String = Int32` at the top
///   of a file is accepted on Julia 1.12.6 with no warning, and every
///   later `name::String` then declares an `Int32` field. A `typedef long
///   String;` would do exactly that, and the file would load.
///
/// The IDL name is not lost when this fires: it is written into the
/// comment above the declaration.
///
/// A leading underscore is left alone. `_d` is a legal Julia field name
/// and renaming it would make the generated name differ from the IDL for
/// no reason.
#[must_use]
pub fn escape_name(name: &str) -> String {
    if KEYWORDS.contains(&name) || WRITTEN_NAMES.contains(&name) {
        let mut out = name.to_string();
        out.push('_');
        return out;
    }
    name.to_string()
}

/// The name an IDL identifier takes as a struct field.
///
/// Keywords only. A field name lives in the struct's own namespace and
/// shadows nothing: `struct S; Int32::Int32; String::String; end` defines
/// two fields called `Int32` and `String`, and both annotations still
/// resolve to the `Core` types (measured on Julia 1.12.6). Escaping them
/// too would rename a member the wire format names, for a collision that
/// does not exist.
#[must_use]
pub fn escape_field(name: &str) -> String {
    if KEYWORDS.contains(&name) {
        let mut out = name.to_string();
        out.push('_');
        return out;
    }
    name.to_string()
}

/// Julia's keywords.
///
/// The list in the manual's index of reserved words. The words that are
/// only keywords in combination — `abstract type`, `mutable struct`,
/// `primitive type` — are **not** here, and neither are the soft
/// keywords `where`, `in`, `isa`, `outer`, `as` and `public`: all of them
/// are legal field names and legal identifiers, measured rather than
/// assumed (`struct F; type::Int32; in::Int32; where::Int32;
/// mutable::Int32; abstract::Int32; isa::Int32; end` compiles).
pub const KEYWORDS: &[&str] = &[
    "baremodule",
    "begin",
    "break",
    "catch",
    "const",
    "continue",
    "do",
    "else",
    "elseif",
    "end",
    "export",
    "false",
    "for",
    "function",
    "global",
    "if",
    "import",
    "let",
    "local",
    "macro",
    "module",
    "quote",
    "return",
    "struct",
    "true",
    "try",
    "using",
    "while",
];

/// The names a generated Julia file writes, which nothing in it may
/// rebind.
///
/// Not keywords: Julia lets a file shadow any of them, and the file loads.
/// That is the reason they are here. Every field type in a generated file
/// is one of these names, so a declaration that rebound one would change
/// the meaning of every declaration after it — silently, and only for the
/// declarations that came later.
pub const WRITTEN_NAMES: &[&str] = &[
    "Any", "Bool", "Enum", "Float32", "Float64", "Int8", "Int16", "Int32", "Int64", "NTuple",
    "Nothing", "String", "UInt8", "UInt16", "UInt32", "UInt64", "Union", "Vector", "nothing",
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
    fn no_primitive_has_a_platform_defined_width() {
        // `Int` is `Int64` on a 64-bit platform and `Int32` on a 32-bit
        // one (Julia manual, "Integers and Floating-Point Numbers"). A
        // member declared with one would change width between the host
        // that generated the file and the target that runs it.
        for primitive in Primitive::ALL {
            assert!(
                !matches!(primitive.as_str(), "Int" | "UInt" | "BigInt" | "BigFloat"),
                "{} has no fixed width",
                primitive.as_str()
            );
        }
    }

    #[test]
    fn a_keyword_escapes_with_a_trailing_underscore() {
        assert_eq!(escape_name("end"), "end_");
        assert_eq!(escape_name("function"), "function_");
        assert_eq!(escape_name("payload"), "payload");
    }

    #[test]
    fn a_name_the_file_writes_escapes_although_it_would_parse() {
        // `const String = Int32` is accepted with no warning, and every
        // later `::String` then means `Int32`.
        for name in ["String", "Int32", "Vector", "Nothing", "Union", "nothing"] {
            assert_eq!(escape_name(name), alloc::format!("{name}_"));
        }
    }

    #[test]
    fn a_field_escapes_a_keyword_and_keeps_a_type_name() {
        // A field name lives in the struct's namespace and shadows
        // nothing, so `Int32` stays `Int32`; `end` does not parse as a
        // field name at all.
        assert_eq!(escape_field("end"), "end_");
        assert_eq!(escape_field("Int32"), "Int32");
        assert_eq!(escape_field("String"), "String");
    }

    #[test]
    fn a_soft_keyword_is_left_alone_because_it_is_a_legal_name() {
        for name in ["where", "in", "isa", "outer", "type", "mutable", "abstract"] {
            assert_eq!(escape_name(name), name);
            assert_eq!(escape_field(name), name);
        }
    }

    #[test]
    fn a_leading_underscore_is_left_alone() {
        // `_d` is the discriminator's name in every column and is a legal
        // Julia field name.
        assert_eq!(escape_field("_d"), "_d");
        assert_eq!(escape_name("_reserved"), "_reserved");
    }

    #[test]
    fn neither_reserved_list_has_a_duplicate() {
        for list in [KEYWORDS, WRITTEN_NAMES] {
            let mut seen: Vec<&str> = Vec::new();
            for name in list {
                assert!(!seen.contains(name), "{name} is listed twice");
                seen.push(name);
            }
        }
    }

    #[test]
    fn an_array_type_nests_outermost_dimension_first() {
        // `long grid[2][3]` is two rows of three, which in Julia is
        // `NTuple{2, NTuple{3, Int32}}`. Reversing it silently transposes
        // the data.
        let type_ = Type::ntuple(2, Type::ntuple(3, Type::Primitive(Primitive::Int32)));
        let Type::NTuple { length, element } = &type_ else {
            panic!("not an NTuple");
        };
        assert_eq!(**length, Expr::int(2));
        let Type::NTuple { length, .. } = &**element else {
            panic!("the element is not an NTuple");
        };
        assert_eq!(**length, Expr::int(3));
    }

    #[test]
    fn an_optional_puts_the_type_first_and_its_absence_second() {
        let type_ = Type::optional(Type::Primitive(Primitive::Int32));
        let Type::Union(arms) = &type_ else {
            panic!("not a Union");
        };
        assert_eq!(arms.len(), 2);
        assert_eq!(arms[0], Type::Primitive(Primitive::Int32));
        assert_eq!(arms[1], Type::Primitive(Primitive::Nothing));
    }

    #[test]
    fn a_struct_carries_no_mutability_and_a_supertype_only_when_set() {
        let decl = StructDecl {
            body_text: crate::common::BodyText::none(),
            doc: Doc::none(),
            name: Ident::new("Pose"),
            supertype: None,
            fields: alloc::vec![Field {
                doc: Doc::none(),
                name: Ident::new("x"),
                type_: Type::Primitive(Primitive::Int32),
            }],
        };
        assert!(decl.supertype.is_none());
        assert_eq!(decl.fields.len(), 1);
    }
}
