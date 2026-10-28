// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Swift concrete syntax tree. Spec §1.
//!
//! Swift writes a declaration name-first with the type after a colon, so
//! the tree carries no declarator: `[[Int32]]` is a [`Type::Array`]
//! holding a [`Type::Array`] holding a [`Type::Primitive`], and the
//! renderer never has to split a type around a name the way
//! `zerodds_lang_render::c` does. In that it is the Go tree's shape.
//!
//! Four things this tree holds that the Go tree does not, each of them
//! Swift's:
//!
//! 1. **A visibility.** Swift has real access-level keywords and a
//!    default of `internal` — "Internal access enables entities to be
//!    used within any source file from their defining module, but not in
//!    any source file outside of that module" (*The Swift Programming
//!    Language*, "Access Control"). A generated type left at the default
//!    cannot be named by the program that imports it, so
//!    [`crate::common::Visibility`] is on every declaration and the
//!    materialiser sets it.
//! 2. **An initialiser.** Swift synthesises a memberwise initialiser for
//!    a struct, and that initialiser is `internal` even when every
//!    property is `public` (same chapter, "Default Memberwise
//!    Initializers for Structure Types"). A `public struct` with no
//!    written `init` is therefore a type another module can hold and
//!    cannot construct, so [`AggregateDecl::inits`] exists and the
//!    materialiser fills it.
//! 3. **A conformance list.** `OptionSet` is how the standard library
//!    spells a set of bit flags, and it is a protocol conformance rather
//!    than a keyword.
//! 4. **Both `struct` and `class`.** [`Aggregate`] has two spellings
//!    because Swift's only stored form that does not sit inside its
//!    owner is a reference. A value type whose stored property
//!    recursively contains it is rejected — measured, on Swift 6.3:
//!    `value type 'N' cannot have a stored property that recursively
//!    contains it`. `@external` is the annotation that says a member is
//!    stored out of line, and [`Aggregate::Class`] is what carries it.
//!
//! ## What is not here
//!
//! No function bodies beyond the assignments an initialiser is, no
//! statements, no closures, no `actor`, no property wrappers. This tree
//! carries the *types* an IDL file becomes and the method signatures an
//! `interface` declares. Every form has a renderer test, so no arm of
//! `zerodds_lang_render::swift` is code that only claims to work.

use crate::common::{BodyText, Doc, Ident, Literal, Visibility};
use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// A Swift source file.
///
/// One file per IDL file. Swift's unit of namespacing is the *module*,
/// which is a build-system target and not something a generated file can
/// declare, so nested IDL modules cannot nest here as modules; they
/// flatten onto the declared names. The reason a nested type is not used
/// instead is in `zerodds_lang_rules::adapter::swift`, where the
/// flattening is done.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct File {
    /// The leading comment, written at the top of the file.
    pub doc: Doc,
    /// `import` lines, in written order.
    pub imports: Vec<Ident>,
    /// Declarations, in written order.
    ///
    /// Order is the source's and not a dependency walk: "the order in
    /// which declarations appear within a type has no effect on the
    /// program" — a Swift declaration may be named before it is
    /// written, at file scope and inside a type alike.
    pub decls: Vec<Decl>,
}

/// One top-level declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decl {
    /// `struct Name { ... }` or `final class Name { ... }`.
    Aggregate(AggregateDecl),
    /// `enum Name: Raw { case ... }`.
    Enum(EnumDecl),
    /// `typealias Name = T`.
    TypeAlias(TypeAliasDecl),
    /// `let NAME: T = value`.
    Constant(ConstDecl),
    /// `protocol Name { ... }`.
    Protocol(ProtocolDecl),
    /// A free function with a body — what a codec routine is. The
    /// types column emitted none of these; the statement layer below
    /// exists for exactly this declaration.
    Func(FuncDecl),
    /// A comment standing on its own.
    Comment(Doc),
    /// `@verbatim` text, copied into the output unchanged — OMG IDL 4.2
    /// §8.3.5.1 asks for the text "verbatim", so raw lines and never a
    /// comment.
    Verbatim(String),
}

/// `func name(params) -> Return { body }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuncDecl {
    /// Documentation.
    pub doc: Doc,
    /// How visible the declaration is.
    ///
    /// Swift's default is `internal`, which is the *module* and not the
    /// file — so a generated codec with no keyword is callable from
    /// anywhere in the target that compiles it and from nowhere else. A
    /// generated file handed to a consumer as its own module needs
    /// `public`, the same word the aggregates beside it already carry.
    pub visibility: Visibility,
    /// The name declared.
    pub name: Ident,
    /// Parameters, in order. A codec routine's are unlabelled —
    /// [`Param::label`] is `_` and [`Param::internal_name`] carries the
    /// body's name — so the shared walk's positional calls are Swift.
    pub params: Vec<Param>,
    /// Whether the declaration is `throws` — every generated codec
    /// function is: a malformed stream or an out-of-range sample is a
    /// `ZdWireError` the runtime throws, and a function that calls into
    /// the runtime, or into another codec function, has to be `throws`
    /// itself to propagate it rather than swallow it.
    pub throws: bool,
    /// The result type, absent for a function returning nothing.
    pub returns: Option<Type>,
    /// The body.
    pub body: Vec<Stmt>,
}

/// Which of Swift's two nominal aggregate keywords a declaration uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Aggregate {
    /// `struct` — a value type, copied on assignment.
    Struct,
    /// `final class` — a reference type.
    ///
    /// `final` and not a bare `class`, because nothing generated is
    /// subclassed and a non-final public class is an extension point the
    /// IDL did not declare.
    Class,
}

/// `struct Name: Conformance { ... }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AggregateDecl {
    /// Documentation.
    pub doc: Doc,
    /// How visible the declaration is.
    pub visibility: Visibility,
    /// `struct` or `final class`.
    pub kind: Aggregate,
    /// The name declared.
    pub name: Ident,
    /// Generic parameters, in written order. Empty for the usual case.
    pub generics: Vec<Ident>,
    /// Protocols the declaration conforms to, in written order.
    pub conformances: Vec<Ident>,
    /// Stored properties, in written order.
    pub properties: Vec<Property>,
    /// `static let` members, in written order.
    pub statics: Vec<StaticLet>,
    /// Initialisers, in written order.
    pub inits: Vec<Init>,
    /// `@verbatim` text at the start and the end of the body.
    pub body_text: BodyText,
}

/// One stored property.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Property {
    /// Documentation.
    pub doc: Doc,
    /// How visible it is.
    pub visibility: Visibility,
    /// `var` when true, `let` when false.
    pub mutable: bool,
    /// The name, as it will be written — already escaped.
    pub name: Ident,
    /// The type.
    pub type_: Type,
}

/// One `static let` member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticLet {
    /// Documentation.
    pub doc: Doc,
    /// How visible it is.
    pub visibility: Visibility,
    /// The name, already escaped.
    pub name: Ident,
    /// The declared type, absent where the initialiser gives it.
    pub type_: Option<Type>,
    /// The value.
    pub value: Expr,
}

/// One initialiser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Init {
    /// Documentation.
    pub doc: Doc,
    /// How visible it is.
    pub visibility: Visibility,
    /// Parameters, in order.
    pub params: Vec<Param>,
    /// The body: assignments to stored properties, in order.
    pub body: Vec<Stmt>,
}

/// A statement.
///
/// [`Stmt::AssignProperty`] is all an initialiser needs; the other
/// forms are what a codec body writes, and only those — Swift's richer
/// set (`guard`, `switch`, `repeat`, closures) is deliberately not
/// here: a form no walk builds is a renderer arm no test reaches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    /// `self.<property> = <value>`.
    AssignProperty {
        /// The property assigned, already escaped.
        property: Ident,
        /// What it is assigned.
        value: Expr,
    },
    /// `var name: T = value`, `var name = value`, or `var name: T`.
    ///
    /// Always `var`: which locals a codec body reassigns is the walk's
    /// business and not this tree's, and a `let` that is later assigned
    /// does not compile. The probe compiles with `-suppress-warnings`,
    /// so the never-mutated advisory does not fail a build.
    Var {
        /// The name declared.
        name: Ident,
        /// The declared type; absent leaves it to the initialiser.
        type_: Option<Type>,
        /// The initialiser; absent relies on Swift's definite
        /// initialisation to prove every use follows an assignment.
        value: Option<Expr>,
    },
    /// `place = value` — also `_ = value`, which is how Swift discards
    /// a result on purpose.
    Assign {
        /// The place written.
        place: Expr,
        /// The value.
        value: Expr,
    },
    /// An expression evaluated for its effect.
    Expr(Expr),
    /// `return`, with or without a value.
    Return(Option<Expr>),
    /// `if condition { .. } else { .. }`.
    If {
        /// The condition.
        condition: Expr,
        /// The then-branch.
        then: Vec<Stmt>,
        /// The else-branch, absent where there is none.
        otherwise: Option<Vec<Stmt>>,
    },
    /// `for counter in 0..<bound { .. }`.
    ///
    /// The counter's type is the bound's: a `UInt32` count off the wire
    /// makes a `UInt32` counter, which is what keeps every comparison
    /// in the body homogeneous — Swift has no mixed-width integer `<`.
    ForRange {
        /// The counter's name.
        counter: Ident,
        /// The exclusive bound.
        bound: Expr,
        /// The body.
        body: Vec<Stmt>,
    },
    /// `while condition { .. }`.
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

/// One parameter of an initialiser or a method.
///
/// Swift gives a parameter two names: the label written at the call site
/// and the name the body uses (*The Swift Programming Language*,
/// "Functions", "Specifying Argument Labels"). They are the same name
/// unless [`Param::internal_name`] says otherwise, which is needed for
/// exactly one member spelling — see
/// `zerodds_lang_rules::adapter::swift`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    /// The argument label, as written at the call site.
    pub label: Ident,
    /// The name the body uses, when it differs from the label.
    pub internal_name: Option<Ident>,
    /// The type.
    pub type_: Type,
}

/// `enum Name: Raw { case ... }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumDecl {
    /// Documentation.
    pub doc: Doc,
    /// How visible it is.
    pub visibility: Visibility,
    /// The name declared.
    pub name: Ident,
    /// The raw-value type.
    ///
    /// Optional because Swift rejects a raw type on an enumeration with
    /// no cases — measured, on Swift 6.3: `an enum with no cases cannot
    /// declare a raw type`.
    pub raw_type: Option<Type>,
    /// The cases, in written order.
    pub cases: Vec<EnumCase>,
}

/// One case of an enumeration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumCase {
    /// Documentation.
    pub doc: Doc,
    /// The name, already escaped.
    pub name: Ident,
    /// The raw value, written out.
    pub raw_value: Option<Expr>,
}

/// `typealias Name = T`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeAliasDecl {
    /// Documentation.
    pub doc: Doc,
    /// How visible it is.
    pub visibility: Visibility,
    /// The name declared.
    pub name: Ident,
    /// What it names.
    pub type_: Type,
}

/// `let NAME: T = value`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstDecl {
    /// Documentation.
    pub doc: Doc,
    /// How visible it is.
    pub visibility: Visibility,
    /// The name declared.
    pub name: Ident,
    /// The declared type, written out so the literal cannot pick another.
    pub type_: Option<Type>,
    /// The value.
    pub value: Expr,
}

/// `protocol Name { ... }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolDecl {
    /// Documentation.
    pub doc: Doc,
    /// How visible it is.
    pub visibility: Visibility,
    /// The name declared.
    pub name: Ident,
    /// The requirements, in written order.
    pub methods: Vec<Method>,
}

/// One method requirement of a protocol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Method {
    /// Documentation.
    pub doc: Doc,
    /// The name, already escaped.
    pub name: Ident,
    /// Parameters, in order.
    pub params: Vec<Param>,
    /// Whether the requirement is `throws` — DDS-RPC 1.0's `raises`,
    /// carried as a real Swift keyword rather than only documented,
    /// since Swift has one to carry it in.
    pub throws: bool,
    /// The result, absent for an operation returning `void`.
    pub returns: Option<Type>,
}

/// A Swift type, as it will be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    /// A standard-library type this column relies on.
    Primitive(Primitive),
    /// A declared name.
    Named(Ident),
    /// `Base<A, B>`.
    Generic {
        /// The generic type's name.
        base: Ident,
        /// The arguments, in order.
        arguments: Vec<Type>,
    },
    /// `[T]`.
    ///
    /// The sugar and not `Array<T>`: `[T]` is defined to mean
    /// `Swift.Array<T>` whatever else the file declares, so it cannot be
    /// captured by a generated type that happens to be called `Array`.
    Array(Box<Type>),
    /// `T?`.
    ///
    /// The sugar for `Swift.Optional<T>`, for the same reason.
    Optional(Box<Type>),
}

impl Type {
    /// A type spelled by one name.
    #[must_use]
    pub fn named(text: &str) -> Self {
        Self::Named(Ident::new(text))
    }

    /// `[T]`.
    #[must_use]
    pub fn array(element: Self) -> Self {
        Self::Array(Box::new(element))
    }

    /// `T?`.
    #[must_use]
    pub fn optional(inner: Self) -> Self {
        Self::Optional(Box::new(inner))
    }

    /// `Base<T>`.
    #[must_use]
    pub fn generic(base: &str, arguments: Vec<Self>) -> Self {
        Self::Generic {
            base: Ident::new(base),
            arguments,
        }
    }
}

/// A standard-library type a generated file names.
///
/// `Int` and `UInt` are **not** here and that is the point: their width
/// is the platform's — "Int has the same size as the current platform's
/// native word size" (*The Swift Programming Language*, "The Basics") —
/// so a member declared with one would be four octets on a 32-bit target
/// and eight on a 64-bit one, for a wire format that fixes every width.
///
/// `Float80` is not here either, and for a harder reason: it exists only
/// on x86. Measured, on `arm64-apple-macosx`: `'Float80' is unavailable:
/// Float80 is not available on target platform.` A column that spelled
/// `long double` that way would not compile on Apple silicon at all.
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
    /// `Float` — binary32.
    Float,
    /// `Double` — binary64.
    Double,
    /// `String`
    String,
    /// `Any`
    Any,
}

impl Primitive {
    /// Every standard-library type the tree can hold.
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
        Self::Float,
        Self::Double,
        Self::String,
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
            Self::Float => "Float",
            Self::Double => "Double",
            Self::String => "String",
            Self::Any => "Any",
        }
    }
}

/// An expression.
///
/// The forms a constant, a raw value and an option-set flag take, plus
/// what a codec body needs. Nothing here decides a wire question.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    /// A literal.
    Literal(Literal),
    /// A name.
    Name(Ident),
    /// `a op b`.
    Binary {
        /// The operator, as written.
        op: &'static str,
        /// The left operand.
        lhs: Box<Expr>,
        /// The right operand.
        rhs: Box<Expr>,
    },
    /// `Callee(label: value, ...)`.
    Call {
        /// What is called — a type, for an initialiser call.
        callee: Ident,
        /// The arguments, in order.
        args: Vec<Argument>,
    },
    /// `(a)` — parentheses written on purpose.
    ///
    /// The tree carries no operator precedence, so a builder that needs
    /// a bracket says so.
    Paren(Box<Expr>),
    /// `value.name` — a member access.
    Member {
        /// The value accessed.
        value: Box<Expr>,
        /// The name accessed, already escaped.
        name: Ident,
    },
    /// `receiver.name(args)` or, with no receiver, `name(args)`.
    Invoke {
        /// The receiver, absent for a free function.
        receiver: Option<Box<Expr>>,
        /// The method or function name.
        name: Ident,
        /// The arguments, in order.
        args: Vec<Argument>,
    },
    /// `value[index]`.
    Index {
        /// The value indexed.
        value: Box<Expr>,
        /// The index.
        index: Box<Expr>,
    },
    /// `op value` — `!`, `-` and `~`, written as given with no space.
    Unary {
        /// The operator, as written.
        op: &'static str,
        /// The operand.
        value: Box<Expr>,
    },
    /// `value!` — the force unwrap.
    ///
    /// Written only where a guard has already answered the presence
    /// question: an `@optional` member inside its own presence branch,
    /// a union branch the discriminator selects.
    ForceUnwrap(Box<Expr>),
    /// `[a, b, c]` — an array literal. Empty is `[]`, typed by its
    /// position.
    ArrayLit(Vec<Expr>),
    /// `[T](repeating: value, count: n)` — an array of `n` copies.
    ///
    /// What a reader sizes a map's entry storage with: Swift has no
    /// uninitialised array slot, so the room is made of default-valued
    /// entries and the fill writes into them.
    RepeatingArray {
        /// The element type.
        element: Box<Type>,
        /// The repeated value.
        repeated: Box<Expr>,
        /// The element count.
        count: Box<Expr>,
    },
    /// `T.self` — the metatype value a conversion helper takes.
    TypeSelf(Box<Type>),
    /// `try value` — a call into a throwing runtime function or a
    /// throwing generated codec.
    ///
    /// Always written at the front of whatever expression it is part
    /// of: Swift refuses `try` to the right of a non-assignment
    /// operator, so a builder that finds one nested inside a `Binary`
    /// or `Unary` it is composing has to hoist it there instead of
    /// leaving it where the throwing call sits. A call argument and the
    /// value of an assignment need no hoisting — `try` there is already
    /// at the front of its own sub-expression — which is why this node
    /// only ever wraps the *outermost* expression a throwing call
    /// reached, never a bare operand.
    Try(Box<Expr>),
}

impl Expr {
    /// An integer literal.
    #[must_use]
    pub fn int(value: i128) -> Self {
        Self::Literal(Literal::int(value))
    }
}

/// One argument of a call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Argument {
    /// The label, absent where the parameter declared `_`.
    pub label: Option<Ident>,
    /// The value.
    pub value: Expr,
}

/// The name an IDL identifier takes as a Swift *value* name.
///
/// Back quotes where the identifier is a Swift keyword, and the
/// identifier unchanged otherwise. "To use a reserved word as an
/// identifier, put a backtick (`` ` ``) before and after it" (*The Swift
/// Programming Language*, "Lexical Structure", "Identifiers").
///
/// Back quotes and **not** a trailing underscore, which is what the C,
/// Go and Python columns have to use. The difference is that this escape
/// inverts: `` `default` `` *is* the identifier `default`, so the IDL
/// spelling survives into the Swift source unchanged. A member's ID
/// under `@autoid(HASH)` and under `@hashid` is the first four octets of
/// the MD5 of its **IDL** name (DDS-XTypes 1.3 §7.3.1.2.1.1), and the Go
/// column carries a struct tag on every field precisely because
/// `type` → `Type_` cannot be read back. Nothing here has to.
///
/// The second consequence is that this escape cannot create a collision.
/// `type` and `type_` are two IDL names and stay two Swift names, where
/// in Go they are both `Type_` and one of the two members is
/// unreachable.
#[must_use]
pub fn escape_name(name: &str) -> String {
    if RESERVED.contains(&name) {
        let mut out = String::from("`");
        out.push_str(name);
        out.push('`');
        return out;
    }
    name.to_string()
}

/// The name an IDL identifier takes as a Swift *declared type* name.
///
/// Back quotes for a keyword, as [`escape_name`] does — and a trailing
/// underscore for a standard-library type the generated file itself
/// relies on.
///
/// The underscore is needed because a back quote does not rename. A file
/// declaring `public struct Bool` and, elsewhere, a member of IDL type
/// `boolean` compiles: the member's `Bool` resolves to the *declared*
/// type, because unqualified lookup finds the module's own declaration
/// before the standard library's. Measured, on Swift 6.3 — the file is
/// accepted with no diagnostic at all, which is the silent substitution
/// this project is arranged against.
///
/// Not every name on the list is reachable from IDL. `String` and
/// `Int32` are not: IDL compares identifiers without regard to case
/// (IDL 4.2 §7.2.3), so both collide with the keywords `string` and
/// `int32` and the frontend refuses them. They stay on the list because
/// the list is about what this column *writes*, and a frontend rule is
/// not the place this column's guarantee should rest.
///
/// The IDL name is not lost when this fires: the materialiser writes it
/// into the comment above the declaration.
#[must_use]
pub fn escape_type_name(name: &str) -> String {
    if STDLIB_TYPES.contains(&name) {
        let mut out = name.to_string();
        out.push('_');
        return out;
    }
    escape_name(name)
}

/// Whether a name is a Swift reserved word.
#[must_use]
pub fn is_reserved(name: &str) -> bool {
    RESERVED.contains(&name)
}

/// Every reserved word an escape has to put back quotes round.
///
/// The keyword list of *The Swift Programming Language*, "Lexical
/// Structure", "Keywords and Punctuation": the keywords used in
/// declarations, in statements, and in expressions and types. The
/// keywords "reserved in particular contexts" — `get`, `set`, `willSet`,
/// `some`, `Type`, `Protocol` and the rest — are deliberately absent:
/// they are ordinary identifiers outside their context, and back quotes
/// round them would put a spelling in the file that no reader expects
/// and that the compiler does not require.
///
/// `_` is a keyword and not in this list either, because it cannot be
/// escaped into an identifier; an IDL member named `_` is not something
/// IDL 4.2 §7.2.3 admits.
pub const RESERVED: &[&str] = &[
    // Keywords used in declarations.
    "associatedtype",
    "borrowing",
    "class",
    "consuming",
    "deinit",
    "enum",
    "extension",
    "fileprivate",
    "func",
    "import",
    "init",
    "inout",
    "internal",
    "let",
    "macro",
    "nonisolated",
    "open",
    "operator",
    "package",
    "precedencegroup",
    "private",
    "protocol",
    "public",
    "rethrows",
    "static",
    "struct",
    "subscript",
    "typealias",
    "var",
    // Keywords used in statements.
    "break",
    "case",
    "catch",
    "continue",
    "default",
    "defer",
    "do",
    "else",
    "fallthrough",
    "for",
    "guard",
    "if",
    "in",
    "repeat",
    "return",
    "throw",
    "switch",
    "where",
    "while",
    // Keywords used in expressions and types.
    "Any",
    "as",
    "await",
    "false",
    "is",
    "nil",
    "self",
    "Self",
    "super",
    "throws",
    "true",
    "try",
];

/// The standard-library type names a generated file relies on.
///
/// Every one of them is written by [`Primitive::as_str`] or by the
/// collection sugar, so a declaration that took one of these names would
/// be found by unqualified lookup ahead of the standard library's. The
/// list is closed on purpose: it is not "everything in the standard
/// library", it is what this column writes.
pub const STDLIB_TYPES: &[&str] = &[
    "Bool",
    "Int8",
    "Int16",
    "Int32",
    "Int64",
    "UInt8",
    "UInt16",
    "UInt32",
    "UInt64",
    "Float",
    "Double",
    "String",
    "Array",
    "Optional",
    "OptionSet",
    "RawRepresentable",
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
        // `Int` and `UInt` are the platform's word size (*The Swift
        // Programming Language*, "The Basics"), for a wire format that
        // fixes every width.
        for primitive in Primitive::ALL {
            assert!(
                !matches!(primitive.as_str(), "Int" | "UInt"),
                "{} has no fixed width",
                primitive.as_str()
            );
        }
    }

    #[test]
    fn no_primitive_is_float80_because_it_does_not_exist_on_arm() {
        // Measured, on arm64-apple-macosx: `'Float80' is unavailable:
        // Float80 is not available on target platform.` A `long double`
        // spelled that way would not compile on Apple silicon.
        for primitive in Primitive::ALL {
            assert_ne!(primitive.as_str(), "Float80");
        }
    }

    #[test]
    fn a_keyword_escapes_with_back_quotes_and_keeps_its_spelling() {
        assert_eq!(escape_name("default"), "`default`");
        assert_eq!(escape_name("class"), "`class`");
        assert_eq!(escape_name("self"), "`self`");
        assert_eq!(escape_name("payload"), "payload");
    }

    #[test]
    fn the_escape_inverts_so_two_idl_names_stay_two_swift_names() {
        // The Go column's escape does not: `type` and `type_` both reach
        // `Type_`, and one of the two members is then unreachable. Back
        // quotes do not rename, so nothing collides that did not already
        // collide in the IDL.
        assert_ne!(escape_name("default"), escape_name("default_"));
        assert_ne!(escape_name("class"), escape_name("class_"));
    }

    #[test]
    fn a_declared_type_named_after_a_standard_library_type_is_renamed() {
        // Back quotes would not help: `` `String` `` is `String`, and a
        // declaration of it is found ahead of `Swift.String` by every
        // unqualified use in the same module.
        assert_eq!(escape_type_name("String"), "String_");
        assert_eq!(escape_type_name("Int32"), "Int32_");
        assert_eq!(escape_type_name("Array"), "Array_");
        assert_eq!(escape_type_name("Pose"), "Pose");
    }

    #[test]
    fn a_declared_type_named_after_a_keyword_takes_back_quotes() {
        assert_eq!(escape_type_name("default"), "`default`");
        assert_eq!(escape_type_name("protocol"), "`protocol`");
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
    fn the_contextual_keywords_are_not_reserved() {
        // `get`, `set`, `some`, `Type` and `Protocol` are keywords only
        // where the grammar puts them; a member named one of them needs
        // no back quotes and gets none.
        for name in [
            "get", "set", "some", "Type", "Protocol", "optional", "final",
        ] {
            assert!(!is_reserved(name), "{name} is not a reserved word");
            assert_eq!(escape_name(name), name);
        }
    }

    #[test]
    fn an_array_type_nests_outermost_dimension_first() {
        // `long grid[2][3]` is two rows of three, which in Swift is
        // `[[Int32]]`. Swift's array type carries no length, so the
        // nesting is all the shape there is; reversing it silently
        // transposes the data.
        let type_ = Type::array(Type::array(Type::Primitive(Primitive::Int32)));
        let Type::Array(outer) = &type_ else {
            panic!("not an array");
        };
        let Type::Array(inner) = &**outer else {
            panic!("the element is not an array");
        };
        assert_eq!(**inner, Type::Primitive(Primitive::Int32));
    }

    #[test]
    fn an_optional_wraps_rather_than_replaces() {
        let type_ = Type::optional(Type::Primitive(Primitive::UInt8));
        let Type::Optional(inner) = &type_ else {
            panic!("not an optional");
        };
        assert_eq!(**inner, Type::Primitive(Primitive::UInt8));
    }

    #[test]
    fn a_class_and_a_struct_are_two_spellings_of_one_declaration() {
        // The difference is storage: a `struct` sits inside its owner and
        // a `final class` does not, which is the whole reason
        // `Aggregate::Class` exists.
        assert_ne!(Aggregate::Struct, Aggregate::Class);
    }

    #[test]
    fn an_enum_with_no_cases_can_be_built_without_a_raw_type() {
        // Swift rejects the raw type in that case: `an enum with no cases
        // cannot declare a raw type`.
        let decl = EnumDecl {
            doc: Doc::none(),
            visibility: Visibility::Public,
            name: Ident::new("Empty"),
            raw_type: None,
            cases: Vec::new(),
        };
        assert!(decl.raw_type.is_none());
        assert!(decl.cases.is_empty());
    }

    #[test]
    fn a_parameter_can_carry_a_second_name_for_its_body() {
        let param = Param {
            label: Ident::new("`self`"),
            internal_name: Some(Ident::new("zdSelf")),
            type_: Type::Primitive(Primitive::Int32),
        };
        assert_eq!(param.label.as_str(), "`self`");
        assert_eq!(param.internal_name.expect("the name").as_str(), "zdSelf");
    }
}
