// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The TypeScript concrete syntax tree. Spec §1.
//!
//! TypeScript has two declaration spaces that the other four trees do
//! not separate. A name can be a *type* (`interface`, `type`), a *value*
//! (`const`, `class`, `function`) or both at once, and an `interface`
//! that exists only in the type space leaves nothing behind at run time.
//! The tree keeps them apart the way the language does — [`Decl`] has an
//! [`Interface`] arm and a [`Class`] arm, and a materialiser that wants
//! both emits both.
//!
//! The three things this tree has that the Java tree does not:
//!
//! 1. **A type alias.** `type Row = number[]` is a real declaration, so
//!    an IDL `typedef` has a TypeScript form. Java has none and has to
//!    write a comment instead.
//! 2. **A union type.** [`Type::Union`] over object types with a literal
//!    member is a discriminated union, which is what an IDL `union` is;
//!    it needs no class and no `_d`-plus-optional-fields workaround.
//! 3. **A property name that is not an identifier.** [`PropertyName`]
//!    has a quoted form, because `{ "class": 2 }` is legal where
//!    `{ class: 2 }` in a binding position is not.
//!
//! Nothing here decides anything. `readonly`, `export` and `?` are
//! fields a builder sets; a tree that added `export` on its own would be
//! a second place where the output is decided.

use crate::common::{Doc, Ident, Literal, Path};
use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// A TypeScript source file.
///
/// A `.ts` file with a top-level `import` or `export` is a module, and
/// everything in it is in strict mode. That is not a flag here: it is a
/// consequence of the file carrying exported declarations, and it is why
/// `RESERVED` includes the strict-mode-only reserved words.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct File {
    /// The leading comment.
    pub doc: Doc,
    /// `import` lines, in written order.
    pub imports: Vec<Import>,
    /// The declarations, in written order.
    pub decls: Vec<Decl>,
}

/// One `import`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Import {
    /// The module specifier, as written between the quotes.
    pub module: String,
    /// Whether it is `import type`, which is erased entirely.
    pub type_only: bool,
    /// The default binding, for `import d from "m"`.
    pub default: Option<Ident>,
    /// The named bindings, for `import { a, b as c } from "m"`.
    pub names: Vec<ImportName>,
    /// The namespace binding, for `import * as ns from "m"`.
    pub namespace: Option<Ident>,
}

/// One named binding of an `import`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportName {
    /// The name the module exports.
    pub name: Ident,
    /// The local name, for `a as b`.
    pub alias: Option<Ident>,
    /// Whether this one binding is `type`-only.
    pub type_only: bool,
}

/// One declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decl {
    /// `interface Name { ... }` — a type, with no run-time form.
    Interface(Interface),
    /// `class Name { ... }` — a type and a value.
    Class(Class),
    /// `type Name = T`
    TypeAlias(TypeAlias),
    /// `enum Name { ... }` — see [`Enum::const_`] for the two forms.
    Enum(Enum),
    /// `const name: T = value`
    Variable(Variable),
    /// `function name(...) { ... }`
    Function(Function),
    /// `namespace Name { ... }`
    Namespace(Namespace),
    /// A statement at module scope, run when the module is first
    /// imported.
    ///
    /// The one construct a file needs that is neither a declaration nor
    /// inside one: DDS-TS 1.0 §7.4.1 has every generated type end with
    /// `registerType(<Type>Type);`, a call whose whole purpose is the
    /// side effect of putting the descriptor in the process-wide
    /// registry. Written as a `const` binding instead it would need a
    /// name, and the name would be exported from every generated module
    /// for nothing to read.
    Statement(Statement),
    /// A comment on its own line.
    Comment(Doc),
    /// `@verbatim` text, copied into the output unchanged — OMG IDL 4.2
    /// §8.3.5.1 asks for the text "verbatim", so raw lines and never a
    /// comment.
    Verbatim(String),
}

/// `export interface Name<T> extends B { ... }`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interface {
    /// Documentation.
    pub doc: Doc,
    /// Whether it is `export`ed.
    pub exported: bool,
    /// Name.
    pub name: Ident,
    /// Type parameters.
    pub generics: Vec<TypeParam>,
    /// Interfaces extended, in written order.
    pub extends: Vec<Type>,
    /// Its members, in written order.
    pub members: Vec<Signature>,
}

/// `export class Name implements I { ... }`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Class {
    /// Documentation.
    pub doc: Doc,
    /// Whether it is `export`ed.
    pub exported: bool,
    /// Whether it is `abstract`.
    pub abstract_: bool,
    /// Name.
    pub name: Ident,
    /// Type parameters.
    pub generics: Vec<TypeParam>,
    /// The base class.
    pub extends: Option<Type>,
    /// Interfaces implemented, in written order.
    pub implements: Vec<Type>,
    /// Its members, in written order.
    pub members: Vec<ClassMember>,
}

/// `export type Name<T> = ...`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeAlias {
    /// Documentation.
    pub doc: Doc,
    /// Whether it is `export`ed.
    pub exported: bool,
    /// Name.
    pub name: Ident,
    /// Type parameters.
    pub generics: Vec<TypeParam>,
    /// What it aliases.
    pub type_: Type,
}

/// `export enum Name { A = 0 }`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Enum {
    /// Documentation.
    pub doc: Doc,
    /// Whether it is `export`ed.
    pub exported: bool,
    /// Whether it is a `const enum`.
    ///
    /// The difference is a run-time one: a `const enum` has no object at
    /// all and every use is substituted at the call site, so a consumer
    /// compiled under `isolatedModules` — which is what every bundler
    /// does — cannot read one across a module boundary. A caller that
    /// sets this is saying it knows that.
    pub const_: bool,
    /// Name.
    pub name: Ident,
    /// Its members, in written order.
    pub members: Vec<EnumMember>,
}

/// One member of an `enum`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumMember {
    /// Documentation.
    pub doc: Doc,
    /// Name.
    pub name: PropertyName,
    /// Its value, absent for the implicit one.
    pub value: Option<Expr>,
}

/// `export const name: T = value`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variable {
    /// Documentation.
    pub doc: Doc,
    /// Whether it is `export`ed.
    pub exported: bool,
    /// Whether it is `declare`d — a type-space-only announcement.
    pub declare: bool,
    /// `const`, `let` or `var`.
    pub kind: VarKind,
    /// Name.
    pub name: Ident,
    /// The declared type, absent when it is inferred.
    pub type_: Option<Type>,
    /// The initialiser.
    pub value: Option<Expr>,
}

/// How a variable is bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VarKind {
    /// `const`
    Const,
    /// `let`
    Let,
    /// `var`
    Var,
}

impl VarKind {
    /// Every binding form.
    pub const ALL: [Self; 3] = [Self::Const, Self::Let, Self::Var];

    /// How it is written.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Const => "const",
            Self::Let => "let",
            Self::Var => "var",
        }
    }
}

/// `export function name(a: T): R { ... }`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    /// Documentation.
    pub doc: Doc,
    /// Whether it is `export`ed.
    pub exported: bool,
    /// Name.
    pub name: Ident,
    /// Type parameters.
    pub generics: Vec<TypeParam>,
    /// Parameters.
    pub params: Vec<Param>,
    /// The return type, absent when it is inferred.
    pub returns: Option<Type>,
    /// The body, absent for an overload signature.
    pub body: Option<Block>,
}

/// `export namespace Name { ... }`
///
/// A namespace and not a nested module directory: an IDL module is a
/// scope, a directory is a build-system decision, and this tree returns
/// declarations rather than files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Namespace {
    /// Documentation.
    pub doc: Doc,
    /// Whether it is `export`ed.
    pub exported: bool,
    /// Whether it is `declare`d.
    pub declare: bool,
    /// Name.
    pub name: Ident,
    /// What it holds, in written order.
    pub decls: Vec<Decl>,
}

/// One member of an interface or of an inline object type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Signature {
    /// `name: T` or `readonly name?: T`
    Property(Property),
    /// `name(a: T): R`
    Method(MethodSignature),
    /// `[key: K]: V`
    Index(IndexSignature),
    /// A comment on its own line.
    Comment(Doc),
    /// `@verbatim` text, copied into the output unchanged — OMG IDL 4.2
    /// §8.3.5.1 asks for the text "verbatim", so raw lines and never a
    /// comment.
    Verbatim(String),
}

/// `readonly name?: T`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Property {
    /// Documentation.
    pub doc: Doc,
    /// Whether it is `readonly`.
    pub readonly: bool,
    /// Name.
    pub name: PropertyName,
    /// Whether it is `?`, which under `strictNullChecks` widens the type
    /// by `undefined` and lets the property be absent altogether.
    pub optional: bool,
    /// Type.
    pub type_: Type,
}

/// `name(a: T): R` in an interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodSignature {
    /// Documentation.
    pub doc: Doc,
    /// Name.
    pub name: PropertyName,
    /// Whether it is `?`.
    pub optional: bool,
    /// Type parameters.
    pub generics: Vec<TypeParam>,
    /// Parameters.
    pub params: Vec<Param>,
    /// Return type.
    pub returns: Type,
}

/// `[name: K]: V`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexSignature {
    /// Documentation.
    pub doc: Doc,
    /// Whether it is `readonly`.
    pub readonly: bool,
    /// The parameter's name, which is documentation only.
    pub key: Ident,
    /// The key's type, which TypeScript restricts to `string`, `number`
    /// or `symbol`.
    pub key_type: Type,
    /// The value's type.
    pub value_type: Type,
}

/// One member of a class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClassMember {
    /// A field.
    Field(Field),
    /// A method.
    Method(Method),
    /// The constructor.
    Constructor(Constructor),
    /// A comment on its own line.
    Comment(Doc),
    /// `@verbatim` text, copied into the output unchanged — OMG IDL 4.2
    /// §8.3.5.1 asks for the text "verbatim", so raw lines and never a
    /// comment.
    Verbatim(String),
}

/// `public readonly name: T = value`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// Documentation.
    pub doc: Doc,
    /// `public`, `protected` or `private`; absent means the default,
    /// which is public.
    pub accessibility: Option<Accessibility>,
    /// Whether it is `static`.
    pub static_: bool,
    /// Whether it is `readonly`.
    pub readonly: bool,
    /// Name.
    pub name: PropertyName,
    /// Whether it is `?`.
    pub optional: bool,
    /// Whether it carries the definite-assignment assertion `!`.
    ///
    /// `name!: T` tells the compiler the field is assigned elsewhere and
    /// switches off `strictPropertyInitialization` for it. It is an
    /// assertion and not a proof, which is why a generator reaching for
    /// it should be able to say who does the assigning.
    pub definite: bool,
    /// The declared type, absent when it is inferred from the value.
    pub type_: Option<Type>,
    /// The initialiser.
    pub value: Option<Expr>,
}

/// A method of a class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Method {
    /// Documentation.
    pub doc: Doc,
    /// Accessibility.
    pub accessibility: Option<Accessibility>,
    /// Whether it is `static`.
    pub static_: bool,
    /// Whether it is `abstract`.
    pub abstract_: bool,
    /// Name.
    pub name: PropertyName,
    /// Type parameters.
    pub generics: Vec<TypeParam>,
    /// Parameters.
    pub params: Vec<Param>,
    /// Return type, absent when it is inferred.
    pub returns: Option<Type>,
    /// The body, absent for an abstract method.
    pub body: Option<Block>,
}

/// A class's constructor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Constructor {
    /// Documentation.
    pub doc: Doc,
    /// Accessibility.
    pub accessibility: Option<Accessibility>,
    /// Parameters.
    pub params: Vec<Param>,
    /// The body.
    pub body: Block,
}

/// How visible a class member is.
///
/// Not [`crate::common::Visibility`]: that enum's three states are
/// `pub`/`pub(crate)`/private, and TypeScript's middle state is
/// `protected` — visible to a subclass and not to a package, which is a
/// different rule. Mapping one onto the other would make a diff of two
/// backends show an agreement that is not there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Accessibility {
    /// `public`
    Public,
    /// `protected`
    Protected,
    /// `private`
    Private,
}

impl Accessibility {
    /// Every accessibility TypeScript writes.
    pub const ALL: [Self; 3] = [Self::Public, Self::Protected, Self::Private];

    /// How it is written.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Protected => "protected",
            Self::Private => "private",
        }
    }
}

/// One parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    /// Name.
    pub name: Ident,
    /// Whether it is `?`.
    pub optional: bool,
    /// Whether it is `...rest`.
    pub rest: bool,
    /// The declared type.
    pub type_: Option<Type>,
    /// The default value.
    pub default: Option<Expr>,
}

/// One type parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeParam {
    /// Name.
    pub name: Ident,
    /// `extends` bound.
    pub extends: Option<Type>,
    /// `= T` default.
    pub default: Option<Type>,
}

/// The name a property, an enum member or a class member is written
/// under.
///
/// Two forms, because TypeScript accepts more as a property name than as
/// a binding: `class` is a reserved word and `{ class: 2 }` is still
/// legal, but `{ "a-b": 2 }` needs the quotes. Which form a name takes
/// is a materialiser's decision and never a renderer's — a renderer that
/// quoted on its own would change the emitted source without anything in
/// the tree recording that it had.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropertyName {
    /// Written bare: `name: T`.
    Ident(Ident),
    /// Written in quotes: `"name": T`.
    Quoted(String),
}

impl PropertyName {
    /// A bare property name.
    #[must_use]
    pub fn ident(name: &str) -> Self {
        Self::Ident(Ident::new(name))
    }

    /// The text, without whatever quoting it will be written with.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Ident(ident) => ident.as_str(),
            Self::Quoted(text) => text,
        }
    }
}

/// A TypeScript type as it will be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    /// A built-in type keyword.
    Keyword(Keyword),
    /// A named type with any type arguments.
    Named {
        /// Its path; the segments are joined with `.`.
        path: Path,
        /// Its type arguments, in written order.
        args: Vec<Type>,
    },
    /// `T[]`
    Array(Box<Type>),
    /// `readonly T[]`
    ReadonlyArray(Box<Type>),
    /// `[A, B]`
    Tuple(Vec<Type>),
    /// `A | B`
    Union(Vec<Type>),
    /// `A & B`
    Intersection(Vec<Type>),
    /// `{ a: A; b: B }`
    Object(Vec<Signature>),
    /// A literal type: `1`, `"a"`, `true`, `2n`.
    ///
    /// A `bigint` literal type is a [`Literal::Int`] whose text carries
    /// the `n`, because that enum holds the form a value is *written*
    /// in and `2` and `2n` are two different types.
    Literal(Literal),
    /// `(a: A) => R`
    Function {
        /// Parameters.
        params: Vec<Param>,
        /// Return type.
        returns: Box<Type>,
    },
    /// `(T)`, where precedence needs it.
    Parenthesised(Box<Type>),
    /// `typeof value`
    TypeOf(Path),
    /// `keyof T`
    ///
    /// Its own arm and not a named type spelled with a space in it: the
    /// operand is a type and has to be rendered as one, and
    /// `keyof typeof Colour` — the index DDS-TS 1.0 §7.6 writes an
    /// enumeration's alias with — nests a [`Type::TypeOf`] inside it.
    KeyOf(Box<Type>),
    /// `v is T` — a type predicate, in a function's return position.
    ///
    /// Its own arm and not a named type spelled with spaces: a
    /// predicate is the only return type TypeScript reads as a
    /// narrowing, and a generated type guard whose return type were
    /// `boolean` would compile and narrow nothing. DDS-TS 1.0 §7.4.1
    /// requires the guard to be of type
    /// `(v: unknown) => v is <TypeName>`.
    Predicate {
        /// The parameter the predicate narrows.
        param: Ident,
        /// What it narrows to.
        type_: Box<Type>,
    },
    /// `T[K]`
    Indexed {
        /// The type indexed into.
        object: Box<Type>,
        /// The index.
        index: Box<Type>,
    },
}

impl Type {
    /// A named type with no type arguments.
    #[must_use]
    pub fn named(path: Path) -> Self {
        Self::Named {
            path,
            args: Vec::new(),
        }
    }

    /// A named type with type arguments.
    #[must_use]
    pub fn generic(path: Path, args: Vec<Self>) -> Self {
        Self::Named { path, args }
    }

    /// `number`, which is where most of the IDL integers land.
    #[must_use]
    pub const fn number() -> Self {
        Self::Keyword(Keyword::Number)
    }

    /// `string`.
    #[must_use]
    pub const fn string() -> Self {
        Self::Keyword(Keyword::String)
    }

    /// `bigint`, which is where the 64-bit IDL integers land.
    #[must_use]
    pub const fn bigint() -> Self {
        Self::Keyword(Keyword::BigInt)
    }
}

/// A TypeScript type keyword.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Keyword {
    /// `number` — IEEE 754 binary64, so integers past 2^53 - 1 are not
    /// distinct values in it.
    Number,
    /// `bigint` — an arbitrary-precision integer, and the only exact
    /// holder TypeScript has for a 64-bit one.
    BigInt,
    /// `string`
    String,
    /// `boolean`
    Boolean,
    /// `unknown` — a value whose type is not known, which must be
    /// narrowed before it is used.
    Unknown,
    /// `any` — the escape hatch, which switches off checking.
    Any,
    /// `never`
    Never,
    /// `void`
    Void,
    /// `undefined`
    Undefined,
    /// `null`
    Null,
    /// `object`
    Object,
    /// `symbol`
    Symbol,
}

impl Keyword {
    /// Every type keyword the tree can hold.
    pub const ALL: [Self; 12] = [
        Self::Number,
        Self::BigInt,
        Self::String,
        Self::Boolean,
        Self::Unknown,
        Self::Any,
        Self::Never,
        Self::Void,
        Self::Undefined,
        Self::Null,
        Self::Object,
        Self::Symbol,
    ];

    /// How it is written.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Number => "number",
            Self::BigInt => "bigint",
            Self::String => "string",
            Self::Boolean => "boolean",
            Self::Unknown => "unknown",
            Self::Any => "any",
            Self::Never => "never",
            Self::Void => "void",
            Self::Undefined => "undefined",
            Self::Null => "null",
            Self::Object => "object",
            Self::Symbol => "symbol",
        }
    }
}

/// A block of statements.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Block {
    /// The statements, in order.
    pub statements: Vec<Statement>,
}

/// One statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    /// `const name: T = value;`
    Declare {
        /// The binding form.
        kind: VarKind,
        /// Name.
        name: Ident,
        /// The declared type.
        type_: Option<Type>,
        /// The initialiser.
        value: Option<Expr>,
    },
    /// An expression followed by `;`.
    Expr(Expr),
    /// `return expr;`
    Return(Option<Expr>),
    /// `if (cond) { .. } else { .. }`
    If {
        /// Condition.
        condition: Expr,
        /// Then-block.
        then: Block,
        /// Else-block.
        otherwise: Option<Block>,
    },
    /// `for (let i = 0; i < n; i += 1) { .. }`
    ///
    /// Separate from [`Self::ForOf`], which iterates a value: a codec
    /// walk counts to a bound it computed and has no iterable to hand
    /// it. Building one — `Array.from({length: n}, (_, i) => i)` — would
    /// allocate a list per collection to produce indices that are
    /// already there.
    For {
        /// The counter.
        counter: Ident,
        /// The exclusive bound.
        bound: Box<Expr>,
        /// The body.
        body: Block,
    },
    /// `while (condition) { .. }`
    ///
    /// A `@mutable` aggregate's headers run until its delimiter is used
    /// up and a PL_CDR1 list until its sentinel. Neither has a bound,
    /// and a counted loop with the clauses left empty would claim one.
    While {
        /// The condition.
        condition: Box<Expr>,
        /// The body.
        body: Block,
    },
    /// `break;`
    ///
    /// A `@mutable` read loop leaves when its delimiter is used up and
    /// a PL_CDR1 one when it meets its sentinel. Neither end is a
    /// condition the loop head can carry.
    Break,
    /// `for (const name of range) { .. }`
    ForOf {
        /// The binding form.
        kind: VarKind,
        /// The binding.
        binding: Ident,
        /// What is iterated.
        range: Expr,
        /// The body.
        body: Block,
    },
    /// `switch (value) { case ..: }`
    Switch {
        /// What is switched on.
        value: Expr,
        /// The cases.
        cases: Vec<SwitchCase>,
    },
    /// `throw expr;`
    Throw(Expr),
    /// A comment on its own line.
    Comment(Doc),
}

/// One case of a switch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwitchCase {
    /// The labels; empty means `default:`.
    pub labels: Vec<Expr>,
    /// The body.
    pub body: Block,
    /// Whether it ends in `break;`.
    pub breaks: bool,
}

/// An expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    /// A literal.
    Literal(Literal),
    /// A name or qualified name.
    Path(Path),
    /// `[a, b]`
    Array(Vec<Expr>),
    /// `{ a: 1, b: 2 }`
    Object(Vec<(PropertyName, Expr)>),
    /// `new T(args)`
    New {
        /// The class.
        callee: Path,
        /// Arguments.
        args: Vec<Expr>,
    },
    /// `callee(args)`
    Call {
        /// What is called.
        callee: Box<Expr>,
        /// Arguments.
        args: Vec<Expr>,
    },
    /// `object.name`
    Member {
        /// The object.
        object: Box<Expr>,
        /// The property.
        name: Ident,
    },
    /// `object[index]`
    Index {
        /// The object.
        object: Box<Expr>,
        /// The index.
        index: Box<Expr>,
    },
    /// `(a)`
    ///
    /// The tree carries no precedence, so a caller that needs an
    /// operand bracketed says so. `&` binds more loosely than `!==`
    /// here as it does in Java, and `bits & mask !== 0` groups the
    /// wrong way without this.
    Paren(Box<Expr>),
    /// `a op b`
    Binary {
        /// The operator, as written.
        op: &'static str,
        /// Left.
        lhs: Box<Expr>,
        /// Right.
        rhs: Box<Expr>,
    },
    /// `op value`
    Unary {
        /// The operator, as written.
        op: &'static str,
        /// The operand.
        value: Box<Expr>,
    },
    /// `value as T`
    As {
        /// The value.
        value: Box<Expr>,
        /// The type.
        type_: Box<Type>,
    },
    /// `value as const`
    ///
    /// Its own arm and not `as` over a named type: `const` is not a type
    /// and writing it as one would put a name in the tree that resolves
    /// to nothing.
    AsConst(Box<Expr>),
    /// `cond ? a : b`
    Ternary {
        /// The condition.
        condition: Box<Expr>,
        /// The then-value.
        then: Box<Expr>,
        /// The else-value.
        otherwise: Box<Expr>,
    },
    /// `(a, b) => body`
    Arrow {
        /// Parameter names.
        params: Vec<Ident>,
        /// The body as an expression.
        body: Box<Expr>,
    },
    /// `(expr)`, where precedence needs it.
    Parenthesised(Box<Expr>),
    /// `value!` — the non-null assertion.
    ///
    /// Its own arm and not `as T`, because the type it asserts to is
    /// the operand's own with `undefined` removed and a caller writing
    /// the cast would have to know it. What needs it is an indexed read
    /// under `noUncheckedIndexedAccess`, where `v[i]` is `T | undefined`
    /// however the loop is bounded.
    NonNull(Box<Expr>),
    /// `this`
    This,
    /// `undefined`
    Undefined,
}

/// The name a TypeScript keyword collision escapes to.
///
/// A trailing underscore, which is what the Java tree does, so the two
/// generated columns spell an escaped identifier the same way.
///
/// This is for a **binding** position — the name of an interface, a
/// class, a type alias, a namespace, a `const` or a parameter. A
/// property name is a different question and [`escape_member`] answers
/// it, because TypeScript accepts almost every reserved word there and
/// escaping one would lose the IDL's spelling for nothing.
#[must_use]
pub fn escape_keyword(name: &str) -> String {
    if RESERVED.contains(&name) {
        let mut out = name.to_string();
        out.push('_');
        return out;
    }
    name.to_string()
}

/// The name a class member collision escapes to.
///
/// Almost nothing moves. A reserved word is a legal property name — a
/// class may declare `new`, `class` and `typeof` as fields, and an
/// object literal may carry them as keys — so the IDL's spelling
/// survives where the other four columns have to rename.
///
/// `constructor` is the exception, and it is a syntax error rather than
/// a shadowing: a class body parses `constructor` as the constructor
/// even when a type annotation follows, so `constructor: number` does
/// not parse at all. It is escaped in the interface too, because a class
/// that renamed only its own field would no longer implement the
/// interface it claims to.
#[must_use]
pub fn escape_member(name: &str) -> String {
    if RESERVED_MEMBERS.contains(&name) {
        let mut out = name.to_string();
        out.push('_');
        return out;
    }
    name.to_string()
}

/// Whether [`escape_keyword`] would move this name.
///
/// So a materialiser can record the IDL spelling in a comment exactly
/// where it renamed, and nowhere else.
#[must_use]
pub fn is_reserved(name: &str) -> bool {
    RESERVED.contains(&name)
}

/// Whether [`escape_member`] would move this name.
#[must_use]
pub fn is_reserved_member(name: &str) -> bool {
    RESERVED_MEMBERS.contains(&name)
}

/// The words a generated binding may not take.
///
/// Three groups, and the third is the one a list copied out of the
/// ECMAScript grammar would miss:
///
/// 1. ECMAScript 2024 §12.7.2 reserved words, plus the three literals
///    that are not identifiers.
/// 2. The strict-mode-only additions — `implements`, `interface`, `let`,
///    `package`, `private`, `protected`, `public`, `static`, `yield` —
///    because every emitted file carries `export` and is therefore a
///    module, and a module is strict. `await` for the same reason: a
///    module's top level is an async context.
/// 3. TypeScript's **predefined type names**: `any`, `bigint`,
///    `boolean`, `never`, `number`, `object`, `string`, `symbol`,
///    `undefined`, `unknown`. `interface number { … }` is rejected —
///    "Interface name cannot be 'number'" — and `type number = …` would
///    be a declaration that shadows nothing because the built-in wins.
///    They are not reserved words in JavaScript at all, which is why
///    they have to be measured rather than looked up: the list here is
///    what `tsc 6.0.3 --strict` accepted and rejected across the three
///    binding positions this materialiser writes (a type name, a
///    namespace name and a `const` name).
///
/// `readonly` and `as` are in for the same measured reason: neither is a
/// reserved word, and both break a declaration this file emits.
///
/// The contextual keywords that are **not** in: `type`, `namespace`,
/// `declare`, `is`, `from`, `async` and `abstract` are legal binding
/// names, and `interface type { … }` compiles. Renaming them would move
/// an identifier the IDL wrote for no reason a compiler gives.
const RESERVED: &[&str] = &[
    "any",
    "as",
    "await",
    "bigint",
    "boolean",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "enum",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "function",
    "if",
    "implements",
    "import",
    "in",
    "instanceof",
    "interface",
    "let",
    "never",
    "new",
    "null",
    "number",
    "object",
    "package",
    "private",
    "protected",
    "public",
    "readonly",
    "return",
    "static",
    "string",
    "super",
    "switch",
    "symbol",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "undefined",
    "unknown",
    "var",
    "void",
    "while",
    "with",
    "yield",
];

/// The property names a class body cannot declare.
///
/// One, and it is a parse error rather than a name clash: see
/// [`escape_member`].
const RESERVED_MEMBERS: &[&str] = &["constructor"];

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn every_type_keyword_writes_its_own_spelling() {
        let mut seen = Vec::new();
        for keyword in Keyword::ALL {
            assert!(!seen.contains(&keyword.as_str()), "{}", keyword.as_str());
            seen.push(keyword.as_str());
        }
        assert_eq!(seen.len(), 12);
    }

    #[test]
    fn a_reserved_binding_escapes_with_a_trailing_underscore() {
        assert_eq!(escape_keyword("class"), "class_");
        assert_eq!(escape_keyword("package"), "package_");
        assert_eq!(escape_keyword("null"), "null_");
        assert!(is_reserved("class"));
    }

    #[test]
    fn a_contextual_keyword_is_a_legal_binding_and_keeps_its_spelling() {
        // `interface type { … }` compiles; renaming it would move an
        // identifier the IDL wrote for no reason the compiler gives.
        for name in ["type", "namespace", "declare", "is", "from", "async"] {
            assert_eq!(escape_keyword(name), name);
            assert!(!is_reserved(name));
        }
    }

    #[test]
    fn a_predefined_type_name_is_reserved_although_javascript_does_not_reserve_it() {
        // The group a list copied out of the ECMAScript grammar misses.
        // `interface number { … }` is rejected outright, and `type
        // string = …` declares a name the built-in already holds.
        for name in [
            "any",
            "bigint",
            "boolean",
            "never",
            "number",
            "object",
            "string",
            "symbol",
            "undefined",
            "unknown",
            "readonly",
            "as",
        ] {
            assert!(is_reserved(name), "{name}");
            assert_eq!(escape_keyword(name), alloc::format!("{name}_"));
        }
    }

    #[test]
    fn a_reserved_word_is_still_a_legal_member_name() {
        // The property space is not the binding space: `class Sample {
        // new: number = 0 }` parses. The IDL's spelling survives.
        for name in ["new", "class", "delete", "static", "typeof", "function"] {
            assert_eq!(escape_member(name), name);
            assert!(!is_reserved_member(name));
        }
    }

    #[test]
    fn only_constructor_moves_in_the_member_space() {
        // `constructor: number` does not parse in a class body — the
        // parser has already committed to a constructor by the colon.
        assert_eq!(escape_member("constructor"), "constructor_");
        assert!(is_reserved_member("constructor"));
    }

    #[test]
    fn a_property_name_keeps_its_text_whichever_form_it_takes() {
        assert_eq!(PropertyName::ident("value").as_str(), "value");
        assert_eq!(PropertyName::Quoted("a-b".to_string()).as_str(), "a-b");
    }

    #[test]
    fn a_bigint_literal_type_carries_its_suffix_in_the_written_form() {
        // `2` and `2n` are two types, and `Literal` holds the form a
        // value is written in rather than the value.
        let narrow = Type::Literal(Literal::Int("2".to_string()));
        let wide = Type::Literal(Literal::Int("2n".to_string()));
        assert_ne!(narrow, wide);
    }

    #[test]
    fn accessibility_is_its_own_enum_and_not_the_common_visibility() {
        // The middle state differs: `protected` is visible to a
        // subclass, `pub(crate)` to a compilation unit.
        let mut seen = Vec::new();
        for level in Accessibility::ALL {
            assert!(!seen.contains(&level.as_str()));
            seen.push(level.as_str());
        }
        assert_eq!(seen, alloc::vec!["public", "protected", "private"]);
    }
}
