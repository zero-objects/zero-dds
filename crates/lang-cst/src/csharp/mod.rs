// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The C# concrete syntax tree. Spec §1.
//!
//! C# has what Java has and three things Java does not, and all three
//! change what a tree for it must hold:
//!
//! 1. **Namespaces**, which are a scope without being a type. A file may
//!    declare many top-level types, and the file's name is not the public
//!    type's name — so there is no outer class here and [`Namespace`] is
//!    an item in its own right.
//! 2. **Unsigned integers**, so [`Predefined`] carries `byte`, `ushort`,
//!    `uint` and `ulong` beside their signed counterparts. The IDL widths
//!    map one to one and nothing has to be held as a pattern.
//! 3. **`global::`**, which roots a qualified name at the global
//!    namespace and cannot be captured by anything in scope.
//!    [`crate::common::Path::rooted`] is that flag; the renderer writes
//!    the prefix.
//!
//! Properties, attributes, `params`, `ref`/`out` and expression bodies
//! are here because they are C#'s syntax, not because the IDL mapping
//! reaches for them — the tree is the language's, and the mapping is a
//! rule's.

use crate::common::{Doc, Ident, Literal, Path};
use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// A C# compilation unit.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct File {
    /// The leading comment.
    pub doc: Doc,
    /// `using` directives at compilation-unit level, in written order.
    ///
    /// They precede every declaration: C# requires it, and a using alias
    /// written after a type is a compile error rather than a style
    /// question.
    pub usings: Vec<Using>,
    /// What the file declares, in written order.
    pub items: Vec<Item>,
}

/// One `using` directive.
///
/// The three forms share a node because they share a position: an
/// import, a static import and an alias all sit at the top of a scope.
/// `target` is a [`Type`] and not a [`Path`] so that a C# 12 alias to an
/// array — `using Row = int[];` — is expressible; a plain import is a
/// [`Type::Named`] with no arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Using {
    /// Documentation, rendered as a comment above the directive.
    pub doc: Doc,
    /// The alias name, for `using Name = target;`.
    pub alias: Option<Ident>,
    /// Whether it is `using static`.
    pub static_: bool,
    /// Whether it is a `global using`.
    pub global: bool,
    /// What is imported or aliased.
    pub target: Type,
}

/// One item at file or namespace level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    /// `namespace N { ... }`
    Namespace(Namespace),
    /// A type declaration.
    Type(TypeDecl),
    /// A comment on its own line.
    Comment(Doc),
    /// `@verbatim` text, copied into the output unchanged.
    ///
    /// OMG IDL 4.2 §8.3.5.1: the annotation's `text` "shall be copied
    /// verbatim" into the generated artifact — raw lines, never a
    /// comment. One node per directive, lines split on `\n`.
    Verbatim(String),
}

/// `namespace A.B { ... }`
///
/// The name is a whole [`Path`]: C# writes `namespace a.b` as one
/// declaration, and a tree that could only nest one segment at a time
/// would make that spelling unreachable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Namespace {
    /// Documentation.
    pub doc: Doc,
    /// The name, one or more segments.
    pub name: Path,
    /// `using` directives scoped to this namespace body.
    pub usings: Vec<Using>,
    /// What it declares.
    pub items: Vec<Item>,
}

/// One type declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeDecl {
    /// `class Name { ... }`
    Class(Class),
    /// `struct Name { ... }`
    Struct(Struct),
    /// `interface IName { ... }`
    Interface(Interface),
    /// `enum Name : int { ... }`
    Enum(Enum),
    /// `record Name(..) { ... }`
    Record(Record),
    /// `delegate R Name(..);`
    Delegate(Delegate),
}

impl TypeDecl {
    /// The name it declares.
    #[must_use]
    pub const fn name(&self) -> &Ident {
        match self {
            Self::Class(node) => &node.name,
            Self::Struct(node) => &node.name,
            Self::Interface(node) => &node.name,
            Self::Enum(node) => &node.name,
            Self::Record(node) => &node.name,
            Self::Delegate(node) => &node.name,
        }
    }
}

/// `class Name<T> : Base, IFace { ... }`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Class {
    /// Documentation.
    pub doc: Doc,
    /// Attributes, in written order.
    pub attributes: Vec<Attribute>,
    /// Modifiers, in written order.
    pub modifiers: Vec<Modifier>,
    /// Name.
    pub name: Ident,
    /// Type parameters.
    pub generics: Vec<TypeParam>,
    /// The base type and interfaces, in written order.
    pub bases: Vec<Type>,
    /// `where` clauses.
    pub constraints: Vec<TypeConstraint>,
    /// Members, in written order.
    pub members: Vec<Member>,
}

/// `struct Name { ... }`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Struct {
    /// Documentation.
    pub doc: Doc,
    /// Attributes.
    pub attributes: Vec<Attribute>,
    /// Modifiers.
    pub modifiers: Vec<Modifier>,
    /// Name.
    pub name: Ident,
    /// Type parameters.
    pub generics: Vec<TypeParam>,
    /// Interfaces implemented.
    pub bases: Vec<Type>,
    /// `where` clauses.
    pub constraints: Vec<TypeConstraint>,
    /// Members.
    pub members: Vec<Member>,
}

/// `interface IName { ... }`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interface {
    /// Documentation.
    pub doc: Doc,
    /// Attributes.
    pub attributes: Vec<Attribute>,
    /// Modifiers.
    pub modifiers: Vec<Modifier>,
    /// Name.
    pub name: Ident,
    /// Type parameters.
    pub generics: Vec<TypeParam>,
    /// Interfaces extended.
    pub bases: Vec<Type>,
    /// `where` clauses.
    pub constraints: Vec<TypeConstraint>,
    /// Members.
    pub members: Vec<Member>,
}

/// `enum Name : int { A = 0, B = 1 }`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Enum {
    /// Documentation.
    pub doc: Doc,
    /// Attributes.
    pub attributes: Vec<Attribute>,
    /// Modifiers.
    pub modifiers: Vec<Modifier>,
    /// Name.
    pub name: Ident,
    /// The underlying integral type, absent for the `int` default.
    ///
    /// Written even when it is `int`, if the tree says so: an
    /// enumeration whose width reaches the wire should state it rather
    /// than rely on a language default.
    pub underlying: Option<Predefined>,
    /// The members, in written order.
    pub members: Vec<EnumMember>,
}

/// One enum member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumMember {
    /// Documentation.
    pub doc: Doc,
    /// Attributes.
    pub attributes: Vec<Attribute>,
    /// Name.
    pub name: Ident,
    /// Its value, absent when the language's implicit numbering is meant.
    pub value: Option<Expr>,
}

/// `record class Name(components) : Base { ... }`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    /// Documentation.
    pub doc: Doc,
    /// Attributes.
    pub attributes: Vec<Attribute>,
    /// Modifiers.
    pub modifiers: Vec<Modifier>,
    /// Whether it is a `record class` or a `record struct`.
    pub kind: RecordKind,
    /// Name.
    pub name: Ident,
    /// Type parameters.
    pub generics: Vec<TypeParam>,
    /// The positional parameter list, absent when there is none.
    pub parameters: Option<Vec<Param>>,
    /// Base type and interfaces.
    pub bases: Vec<Type>,
    /// `where` clauses.
    pub constraints: Vec<TypeConstraint>,
    /// Members; an empty body renders as `;`.
    pub members: Vec<Member>,
}

/// Which kind of record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecordKind {
    /// `record class`
    Class,
    /// `record struct`
    Struct,
}

impl RecordKind {
    /// How it is written.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Class => "record class",
            Self::Struct => "record struct",
        }
    }
}

/// `delegate R Name(params);`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delegate {
    /// Documentation.
    pub doc: Doc,
    /// Attributes.
    pub attributes: Vec<Attribute>,
    /// Modifiers.
    pub modifiers: Vec<Modifier>,
    /// Return type.
    pub returns: Type,
    /// Name.
    pub name: Ident,
    /// Type parameters.
    pub generics: Vec<TypeParam>,
    /// Parameters.
    pub params: Vec<Param>,
    /// `where` clauses.
    pub constraints: Vec<TypeConstraint>,
}

/// One member of a type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Member {
    /// A field.
    Field(Field),
    /// A property.
    Property(Property),
    /// A method.
    Method(Method),
    /// A constructor.
    Constructor(Constructor),
    /// A nested type.
    Nested(Box<TypeDecl>),
    /// A comment on its own line.
    Comment(Doc),
    /// `@verbatim` text inside the type body, copied unchanged.
    ///
    /// OMG IDL 4.2 §8.3.5.1 `begin-declaration` / `end-declaration`:
    /// the text lands inside the annotated declaration's scope.
    Verbatim(String),
}

/// `public int Name = value;`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// Documentation.
    pub doc: Doc,
    /// Attributes.
    pub attributes: Vec<Attribute>,
    /// Modifiers.
    pub modifiers: Vec<Modifier>,
    /// Type.
    pub type_: Type,
    /// Name.
    pub name: Ident,
    /// Initialiser.
    pub value: Option<Expr>,
}

/// `public int Name { get; set; }`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Property {
    /// Documentation.
    pub doc: Doc,
    /// Attributes.
    pub attributes: Vec<Attribute>,
    /// Modifiers.
    pub modifiers: Vec<Modifier>,
    /// Type.
    pub type_: Type,
    /// Name.
    pub name: Ident,
    /// The accessors; empty means the expression body below is used.
    pub accessors: Vec<Accessor>,
    /// `=> expr;`, for an expression-bodied property.
    pub expression: Option<Expr>,
    /// `= expr;`, a property initialiser.
    pub initialiser: Option<Expr>,
}

/// One accessor of a property.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Accessor {
    /// Which accessor it is.
    pub kind: AccessorKind,
    /// Modifiers, such as `private` on a setter.
    pub modifiers: Vec<Modifier>,
    /// A block body.
    pub body: Option<Block>,
    /// `=> expr;`.
    pub expression: Option<Expr>,
}

/// Which accessor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessorKind {
    /// `get`
    Get,
    /// `set`
    Set,
    /// `init`
    Init,
}

impl AccessorKind {
    /// Every accessor kind.
    pub const ALL: [Self; 3] = [Self::Get, Self::Set, Self::Init];

    /// How it is written.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "get",
            Self::Set => "set",
            Self::Init => "init",
        }
    }
}

/// A method.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Method {
    /// Documentation.
    pub doc: Doc,
    /// Attributes.
    pub attributes: Vec<Attribute>,
    /// Modifiers.
    pub modifiers: Vec<Modifier>,
    /// Return type.
    pub returns: Type,
    /// Name.
    pub name: Ident,
    /// Type parameters.
    pub generics: Vec<TypeParam>,
    /// Parameters.
    pub params: Vec<Param>,
    /// `where` clauses.
    pub constraints: Vec<TypeConstraint>,
    /// A block body, absent for an abstract or interface method.
    pub body: Option<Block>,
    /// `=> expr;`, for an expression-bodied method.
    pub expression: Option<Expr>,
}

/// A constructor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Constructor {
    /// Documentation.
    pub doc: Doc,
    /// Attributes.
    pub attributes: Vec<Attribute>,
    /// Modifiers.
    pub modifiers: Vec<Modifier>,
    /// The type's name.
    pub name: Ident,
    /// Parameters.
    pub params: Vec<Param>,
    /// `: base(..)` or `: this(..)`.
    pub initialiser: Option<ConstructorInitialiser>,
    /// The body.
    pub body: Block,
}

/// `: base(args)` or `: this(args)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstructorInitialiser {
    /// Whether it chains to `this` rather than `base`.
    pub this: bool,
    /// The arguments.
    pub args: Vec<Expr>,
}

/// One parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    /// Attributes.
    pub attributes: Vec<Attribute>,
    /// `ref`, `out`, `in`, `params` or `this`.
    pub modifier: Option<ParamModifier>,
    /// Type.
    pub type_: Type,
    /// Name.
    pub name: Ident,
    /// A default value.
    pub default: Option<Expr>,
}

/// How a parameter is passed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParamModifier {
    /// `ref`
    Ref,
    /// `out`
    Out,
    /// `in`
    In,
    /// `params`
    Params,
    /// `this`, on an extension method.
    This,
}

impl ParamModifier {
    /// Every parameter modifier.
    pub const ALL: [Self; 5] = [Self::Ref, Self::Out, Self::In, Self::Params, Self::This];

    /// How it is written.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ref => "ref",
            Self::Out => "out",
            Self::In => "in",
            Self::Params => "params",
            Self::This => "this",
        }
    }
}

/// A modifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Modifier {
    /// `public`
    Public,
    /// `private`
    Private,
    /// `protected`
    Protected,
    /// `internal`
    Internal,
    /// `static`
    Static,
    /// `sealed`
    Sealed,
    /// `abstract`
    Abstract,
    /// `partial`
    Partial,
    /// `readonly`
    Readonly,
    /// `const`
    Const,
    /// `virtual`
    Virtual,
    /// `override`
    Override,
    /// `new`
    New,
    /// `extern`
    Extern,
    /// `unsafe`
    Unsafe,
    /// `async`
    Async,
    /// `volatile`
    Volatile,
    /// `required`
    Required,
    /// `ref`, on a `ref struct`.
    Ref,
    /// `file`
    File,
}

impl Modifier {
    /// Every modifier the tree can hold.
    pub const ALL: [Self; 20] = [
        Self::Public,
        Self::Private,
        Self::Protected,
        Self::Internal,
        Self::Static,
        Self::Sealed,
        Self::Abstract,
        Self::Partial,
        Self::Readonly,
        Self::Const,
        Self::Virtual,
        Self::Override,
        Self::New,
        Self::Extern,
        Self::Unsafe,
        Self::Async,
        Self::Volatile,
        Self::Required,
        Self::Ref,
        Self::File,
    ];

    /// How it is written.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Private => "private",
            Self::Protected => "protected",
            Self::Internal => "internal",
            Self::Static => "static",
            Self::Sealed => "sealed",
            Self::Abstract => "abstract",
            Self::Partial => "partial",
            Self::Readonly => "readonly",
            Self::Const => "const",
            Self::Virtual => "virtual",
            Self::Override => "override",
            Self::New => "new",
            Self::Extern => "extern",
            Self::Unsafe => "unsafe",
            Self::Async => "async",
            Self::Volatile => "volatile",
            Self::Required => "required",
            Self::Ref => "ref",
            Self::File => "file",
        }
    }
}

/// One attribute use: `[Name(args)]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribute {
    /// A target, such as `assembly` or `return`.
    pub target: Option<Ident>,
    /// The attribute's name.
    pub path: Path,
    /// Its arguments, in written order.
    pub args: Vec<AttributeArg>,
}

/// One argument of an attribute use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttributeArg {
    /// A positional argument.
    Positional(Expr),
    /// `Name = value`.
    Named {
        /// The property or field named.
        name: Ident,
        /// Its value.
        value: Expr,
    },
}

/// One type parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeParam {
    /// `in` or `out`, for a variant parameter.
    pub variance: Option<Variance>,
    /// Name.
    pub name: Ident,
}

/// A type parameter's variance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Variance {
    /// `in`
    In,
    /// `out`
    Out,
}

impl Variance {
    /// How it is written.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::In => "in",
            Self::Out => "out",
        }
    }
}

/// One `where T : bounds` clause.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeConstraint {
    /// The parameter constrained.
    pub param: Ident,
    /// The bounds, as written — `class`, `new()` and a type all appear
    /// here, so they are text rather than a type: `new()` is not one.
    pub bounds: Vec<String>,
}

/// A C# type as it will be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    /// A keyword type: `int`, `string`, `object`.
    ///
    /// The keyword and not `System.Int32`: a keyword cannot be captured
    /// by a declaration, and a generated `namespace System` would
    /// otherwise change what `System.Int32` names.
    Predefined(Predefined),
    /// A named type with any type arguments.
    Named {
        /// Its path; a rooted path renders with `global::`.
        path: Path,
        /// Its type arguments, in written order.
        args: Vec<Type>,
    },
    /// `T[]`, or `T[,]` for a rank above one.
    Array {
        /// The element type.
        element: Box<Type>,
        /// How many dimensions this `[]` has; one for `T[]`.
        rank: usize,
    },
    /// `T?`
    Nullable(Box<Type>),
    /// `T*`
    Pointer(Box<Type>),
    /// `(A a, B b)`
    Tuple(Vec<TupleElement>),
    /// `var`, in a local declaration.
    Var,
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

    /// `T[]`, one dimension.
    #[must_use]
    pub fn array(element: Self) -> Self {
        Self::Array {
            element: Box::new(element),
            rank: 1,
        }
    }
}

/// One element of a tuple type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TupleElement {
    /// Its type.
    pub type_: Type,
    /// Its name, when the tuple element is named.
    pub name: Option<Ident>,
}

/// A C# keyword type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Predefined {
    /// `bool`
    Bool,
    /// `sbyte`, signed 8-bit.
    SByte,
    /// `byte`, unsigned 8-bit.
    Byte,
    /// `short`, signed 16-bit.
    Short,
    /// `ushort`, unsigned 16-bit.
    UShort,
    /// `int`, signed 32-bit — which is what an IDL `long` is.
    Int,
    /// `uint`, unsigned 32-bit.
    UInt,
    /// `long`, signed 64-bit — which is what an IDL `long long` is.
    Long,
    /// `ulong`, unsigned 64-bit.
    ULong,
    /// `char`, one UTF-16 code unit.
    Char,
    /// `float`, binary32.
    Float,
    /// `double`, binary64.
    Double,
    /// `decimal`, a 128-bit base-ten type — **not** a binary128.
    Decimal,
    /// `string`
    String,
    /// `object`
    Object,
    /// `void`
    Void,
    /// `nint`
    NInt,
    /// `nuint`
    NUInt,
    /// `dynamic`
    Dynamic,
}

impl Predefined {
    /// Every keyword type the tree can hold.
    pub const ALL: [Self; 19] = [
        Self::Bool,
        Self::SByte,
        Self::Byte,
        Self::Short,
        Self::UShort,
        Self::Int,
        Self::UInt,
        Self::Long,
        Self::ULong,
        Self::Char,
        Self::Float,
        Self::Double,
        Self::Decimal,
        Self::String,
        Self::Object,
        Self::Void,
        Self::NInt,
        Self::NUInt,
        Self::Dynamic,
    ];

    /// How it is written.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::SByte => "sbyte",
            Self::Byte => "byte",
            Self::Short => "short",
            Self::UShort => "ushort",
            Self::Int => "int",
            Self::UInt => "uint",
            Self::Long => "long",
            Self::ULong => "ulong",
            Self::Char => "char",
            Self::Float => "float",
            Self::Double => "double",
            Self::Decimal => "decimal",
            Self::String => "string",
            Self::Object => "object",
            Self::Void => "void",
            Self::NInt => "nint",
            Self::NUInt => "nuint",
            Self::Dynamic => "dynamic",
        }
    }

    /// Whether it is a value type, which decides whether `T?` is a
    /// `Nullable<T>` or a nullable reference annotation.
    ///
    /// `string`, `object` and `dynamic` are references; every other
    /// keyword type is a value. A caller building an `@optional` member
    /// needs the distinction: `int?` is a different type from `int`,
    /// while `string?` is the same type with an annotation the compiler
    /// only reads inside a `#nullable enable` context.
    #[must_use]
    pub const fn is_value_type(self) -> bool {
        !matches!(
            self,
            Self::String | Self::Object | Self::Dynamic | Self::Void
        )
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
    /// A local declaration.
    Declare {
        /// Type; [`Type::Var`] for `var`.
        type_: Type,
        /// Name.
        name: Ident,
        /// Initialiser.
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
    /// `foreach (T name in range) { .. }`
    ForEach {
        /// The binding's type.
        type_: Type,
        /// The binding.
        binding: Ident,
        /// What is iterated.
        range: Expr,
        /// The body.
        body: Block,
    },
    /// `for (init; condition; step) { .. }`
    For {
        /// The initialiser statement.
        init: Box<Statement>,
        /// The condition.
        condition: Expr,
        /// The step expression.
        step: Expr,
        /// The body.
        body: Block,
    },
    /// `while (cond) { .. }`
    While {
        /// The condition.
        condition: Expr,
        /// The body.
        body: Block,
    },
    /// `switch (value) { case ..: }`
    Switch {
        /// What is switched on.
        value: Expr,
        /// The sections.
        sections: Vec<SwitchSection>,
    },
    /// `throw expr;`
    Throw(Expr),
    /// `using (decl) { .. }`
    Using {
        /// The resource declaration or expression.
        resource: Box<Statement>,
        /// The body.
        body: Block,
    },
    /// A nested block.
    Block(Block),
    /// `break;`
    Break,
    /// A comment on its own line.
    Comment(Doc),
}

/// One section of a switch.
///
/// C# requires every reachable section to end in a jump — `break`,
/// `return` or `throw` — so `breaks` is not decoration: a section
/// without one does not compile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwitchSection {
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
    /// A name or qualified name; a rooted path renders with `global::`.
    Path(Path),
    /// `receiver.name(args)`
    Invoke {
        /// The receiver, absent for an unqualified call.
        receiver: Option<Box<Expr>>,
        /// The method.
        name: Ident,
        /// Arguments.
        args: Vec<Expr>,
    },
    /// `value.name`
    Member {
        /// The value.
        value: Box<Expr>,
        /// The member.
        name: Ident,
    },
    /// `a[i]`
    Index {
        /// The indexed value.
        value: Box<Expr>,
        /// The index.
        index: Box<Expr>,
    },
    /// `new T(args)`, with an optional object initialiser.
    New {
        /// The type.
        type_: Box<Type>,
        /// Arguments.
        args: Vec<Expr>,
        /// `{ Name = value, .. }`.
        initialiser: Vec<(Ident, Expr)>,
    },
    /// `new T[n]`, `new T[]{ a, b }`, or the jagged `new T[n][]`.
    NewArray {
        /// The innermost element type.
        element: Box<Type>,
        /// The length, for `new T[n]`.
        length: Option<Box<Expr>>,
        /// The elements, for the initialiser form.
        elements: Vec<Expr>,
        /// How many empty `[]` follow the sized dimension.
        ///
        /// A C# jagged array is allocated outermost-first and the inner
        /// ranks are left empty: `new int[3][]` and never
        /// `new int[][3]`, which is not an expression at all. The count
        /// is here rather than folded into `element` because the
        /// brackets go on the far side of the length, which is the one
        /// place C#'s array syntax does not read outside-in.
        ///
        /// Zero for `new T[n]`. The C# mapping of `T[N][M]` is jagged —
        /// `adapter/csharp.rs` gives the reason — so every multi-rank
        /// allocation this tree carries sets it.
        nested: usize,
    },
    /// `a op b`
    Binary {
        /// The operator, as written.
        op: &'static str,
        /// Left.
        lhs: Box<Expr>,
        /// Right.
        rhs: Box<Expr>,
    },
    /// `op a`
    Unary {
        /// The operator, as written.
        op: &'static str,
        /// The operand.
        value: Box<Expr>,
    },
    /// `(T) value`
    Cast {
        /// The target type.
        type_: Box<Type>,
        /// The value.
        value: Box<Expr>,
    },
    /// `value is T`
    Is {
        /// The value.
        value: Box<Expr>,
        /// The type.
        type_: Box<Type>,
    },
    /// `cond ? a : b`
    Conditional {
        /// The condition.
        condition: Box<Expr>,
        /// The then-value.
        then: Box<Expr>,
        /// The else-value.
        otherwise: Box<Expr>,
    },
    /// `target = value`
    Assign {
        /// The target.
        target: Box<Expr>,
        /// The value.
        value: Box<Expr>,
    },
    /// `(expr)`
    Paren(Box<Expr>),
    /// `this`
    This,
    /// `default` or `default(T)`
    Default(Option<Box<Type>>),
    /// `typeof(T)`
    TypeOf(Box<Type>),
    /// `nameof(expr)`
    NameOf(Box<Expr>),
    /// `(a, b) => body`
    Lambda {
        /// Parameter names.
        params: Vec<Ident>,
        /// The body as an expression.
        body: Box<Expr>,
    },
}

/// The name a C# keyword collision escapes to.
///
/// The `@` prefix and not a trailing underscore: §6.4.3 of the C#
/// specification makes `@class` a *verbatim identifier* whose identity is
/// `class`, so the escaped name and the IDL name are the same name — a
/// reflective consumer reading the member back sees `class`, not
/// `class_`. Java has no such form, which is why
/// [`crate::java::escape_keyword`] appends instead.
///
/// Only the reserved words move. The contextual keywords — `var`,
/// `value`, `record`, `async` and the rest — are identifiers wherever an
/// identifier is expected, so escaping them would rename something the
/// compiler never objected to.
#[must_use]
pub fn escape_keyword(name: &str) -> String {
    if RESERVED.contains(&name) {
        let mut out = String::from("@");
        out.push_str(name);
        return out;
    }
    name.to_string()
}

/// Whether `name` is a C# reserved word.
#[must_use]
pub fn is_keyword(name: &str) -> bool {
    RESERVED.contains(&name)
}

/// The C# reserved words a generated identifier may collide with.
///
/// The 77 of ECMA-334 §6.4.4 "Keywords". The contextual keywords of
/// §6.4.4's second table are deliberately absent: they are identifiers
/// wherever one is expected.
const RESERVED: &[&str] = &[
    "abstract",
    "as",
    "base",
    "bool",
    "break",
    "byte",
    "case",
    "catch",
    "char",
    "checked",
    "class",
    "const",
    "continue",
    "decimal",
    "default",
    "delegate",
    "do",
    "double",
    "else",
    "enum",
    "event",
    "explicit",
    "extern",
    "false",
    "finally",
    "fixed",
    "float",
    "for",
    "foreach",
    "goto",
    "if",
    "implicit",
    "in",
    "int",
    "interface",
    "internal",
    "is",
    "lock",
    "long",
    "namespace",
    "new",
    "null",
    "object",
    "operator",
    "out",
    "override",
    "params",
    "private",
    "protected",
    "public",
    "readonly",
    "ref",
    "return",
    "sbyte",
    "sealed",
    "short",
    "sizeof",
    "stackalloc",
    "static",
    "string",
    "struct",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "uint",
    "ulong",
    "unchecked",
    "unsafe",
    "ushort",
    "using",
    "virtual",
    "void",
    "volatile",
    "while",
];

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn every_predefined_type_writes_its_own_spelling() {
        let mut seen = Vec::new();
        for predefined in Predefined::ALL {
            assert!(!seen.contains(&predefined.as_str()));
            seen.push(predefined.as_str());
        }
        assert_eq!(seen.len(), Predefined::ALL.len());
    }

    #[test]
    fn the_signed_and_unsigned_widths_are_both_present() {
        // The difference from Java, and the reason the IDL integer
        // mapping is one to one here: every IDL width has a C# keyword
        // of the same width and the same signedness.
        for (signed, unsigned) in [
            (Predefined::SByte, Predefined::Byte),
            (Predefined::Short, Predefined::UShort),
            (Predefined::Int, Predefined::UInt),
            (Predefined::Long, Predefined::ULong),
        ] {
            assert_ne!(signed.as_str(), unsigned.as_str());
            assert!(signed.is_value_type() && unsigned.is_value_type());
        }
    }

    #[test]
    fn string_and_object_are_reference_types_and_the_numbers_are_not() {
        assert!(!Predefined::String.is_value_type());
        assert!(!Predefined::Object.is_value_type());
        assert!(Predefined::Int.is_value_type());
        assert!(Predefined::Double.is_value_type());
    }

    #[test]
    fn a_keyword_escapes_with_an_at_sign_and_keeps_its_identity() {
        // `@class` *is* the identifier `class`; the escape is lexical.
        assert_eq!(escape_keyword("class"), "@class");
        assert_eq!(escape_keyword("string"), "@string");
        assert_eq!(escape_keyword("value"), "value");
        // A contextual keyword is an identifier and does not move.
        for contextual in ["var", "record", "async", "await", "value", "nameof"] {
            assert_eq!(escape_keyword(contextual), contextual, "{contextual}");
        }
    }

    #[test]
    fn the_reserved_list_has_no_duplicates() {
        let mut seen: Vec<&str> = Vec::new();
        for word in RESERVED {
            assert!(!seen.contains(word), "{word} is listed twice");
            seen.push(word);
        }
        assert_eq!(seen.len(), 77, "ECMA-334 6.4.4 lists 77 keywords");
    }

    #[test]
    fn a_type_declaration_answers_for_its_own_name() {
        let decl = TypeDecl::Class(Class {
            doc: Doc::none(),
            attributes: Vec::new(),
            modifiers: alloc::vec![Modifier::Public],
            name: Ident::new("Pose"),
            generics: Vec::new(),
            bases: Vec::new(),
            constraints: Vec::new(),
            members: Vec::new(),
        });
        assert_eq!(decl.name().as_str(), "Pose");
    }

    #[test]
    fn modifiers_keep_the_order_they_were_written_in() {
        let field = Field {
            doc: Doc::none(),
            attributes: Vec::new(),
            modifiers: alloc::vec![Modifier::Public, Modifier::Static, Modifier::Readonly],
            type_: Type::Predefined(Predefined::Int),
            name: Ident::new("Max"),
            value: None,
        };
        let written: Vec<&str> = field.modifiers.iter().map(|m| m.as_str()).collect();
        assert_eq!(written, alloc::vec!["public", "static", "readonly"]);
    }
}
