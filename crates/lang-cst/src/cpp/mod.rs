// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The C++ concrete syntax tree. Spec §1.
//!
//! C++ has namespaces, templates and access sections; the tree has them
//! as nodes rather than as flags. An access section is where it appears
//! in the class, because that is what the output shows.

use crate::c::Include;
use crate::common::{Doc, Ident, Literal, Path};
use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// A C++ header or source file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct File {
    /// The leading comment.
    pub doc: Doc,
    /// Whether the file opens with `#pragma once`.
    pub pragma_once: bool,
    /// `#include` lines, in written order.
    pub includes: Vec<Include>,
    /// Declarations at file scope, in written order.
    pub decls: Vec<Decl>,
}

/// One declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decl {
    /// `namespace name { ... }`
    Namespace(Namespace),
    /// `class Name { ... };` or `struct Name { ... };`
    Class(Class),
    /// `enum class Name : T { ... };`
    Enum(Enum),
    /// `using Name = ...;`
    Alias(Alias),
    /// A function at namespace scope.
    Function(Function),
    /// A variable at namespace scope.
    Variable(Variable),
    /// `template <..> <decl>`
    Template(Template),
    /// A comment standing on its own.
    Comment(Doc),
    /// `@verbatim` text, copied into the output unchanged.
    ///
    /// OMG IDL 4.2 §8.3.5.1: the annotation's `text` "shall be copied
    /// verbatim" into the generated artifact — raw lines, never a
    /// comment. One node per directive, lines split on `\n`.
    Verbatim(String),
    /// A forward declaration: `class Name;`
    Forward {
        /// Whether it was written `struct`.
        is_struct: bool,
        /// The name.
        name: Ident,
    },
}

/// `namespace name { ... }`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Namespace {
    /// Documentation.
    pub doc: Doc,
    /// Name, empty for an anonymous namespace.
    pub name: Ident,
    /// What is inside.
    pub decls: Vec<Decl>,
}

/// `class Name : bases { sections };`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Class {
    /// Documentation.
    pub doc: Doc,
    /// Whether it is written `struct` rather than `class`.
    pub is_struct: bool,
    /// Name.
    pub name: Ident,
    /// Base classes, in written order.
    pub bases: Vec<Base>,
    /// Access sections, in the order they appear.
    ///
    /// A section rather than a per-member access word: the output has
    /// `public:` once above a run of members, and a tree that stored
    /// access per member would leave the grouping to the renderer.
    pub sections: Vec<Section>,
}

/// One base class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Base {
    /// How it is inherited.
    pub access: Access,
    /// Whether it is `virtual`.
    pub virtual_: bool,
    /// Its name.
    pub path: Path,
}

/// One access section of a class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    /// Which access it opens.
    pub access: Access,
    /// Its members, in written order.
    pub members: Vec<ClassMember>,
}

/// Access specifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Access {
    /// `public`
    Public,
    /// `protected`
    Protected,
    /// `private`
    Private,
}

impl Access {
    /// Every access specifier.
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

/// One member of a class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClassMember {
    /// A data member.
    Field(Field),
    /// A member function.
    Method(Method),
    /// A constructor.
    Constructor(Constructor),
    /// A destructor.
    Destructor(Destructor),
    /// A nested declaration.
    Nested(Box<Decl>),
    /// A `using` inside a class.
    Alias(Alias),
    /// A comment on its own line.
    Comment(Doc),
    /// `@verbatim` text inside the class body, copied unchanged.
    ///
    /// OMG IDL 4.2 §8.3.5.1 `begin-declaration` / `end-declaration`:
    /// the text lands inside the annotated declaration's scope.
    Verbatim(String),
}

/// A data member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// Documentation.
    pub doc: Doc,
    /// Type.
    pub type_: Type,
    /// Name.
    pub name: Ident,
    /// Array dimensions, outermost first.
    pub dims: Vec<Expr>,
    /// A default member initialiser.
    pub default: Option<Expr>,
    /// `static`.
    pub is_static: bool,
    /// `constexpr`.
    pub is_constexpr: bool,
    /// A bitfield width.
    pub bits: Option<u16>,
}

/// A member function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Method {
    /// Documentation.
    pub doc: Doc,
    /// Return type.
    pub returns: Type,
    /// Name.
    pub name: Ident,
    /// Parameters.
    pub params: Vec<Param>,
    /// `const` on the method.
    pub is_const: bool,
    /// `static`.
    pub is_static: bool,
    /// `virtual`.
    pub is_virtual: bool,
    /// `= 0`.
    pub is_pure: bool,
    /// `constexpr`.
    pub is_constexpr: bool,
    /// `noexcept`.
    pub is_noexcept: bool,
    /// `override`.
    pub is_override: bool,
    /// The body, absent for a declaration.
    pub body: Option<Block>,
}

/// A constructor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Constructor {
    /// Documentation.
    pub doc: Doc,
    /// The class's name.
    pub name: Ident,
    /// Parameters.
    pub params: Vec<Param>,
    /// The member initialiser list, in written order.
    pub initialisers: Vec<(Ident, Expr)>,
    /// `explicit`.
    pub is_explicit: bool,
    /// `constexpr`.
    pub is_constexpr: bool,
    /// `noexcept`.
    pub is_noexcept: bool,
    /// The body, or the defaulted or deleted form.
    pub body: FunctionBody,
}

/// A destructor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Destructor {
    /// Documentation.
    pub doc: Doc,
    /// The class's name.
    pub name: Ident,
    /// `virtual`.
    pub is_virtual: bool,
    /// The body, or the defaulted form.
    pub body: FunctionBody,
}

/// What follows a function's signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunctionBody {
    /// `{ ... }`
    Block(Block),
    /// `= default;`
    Defaulted,
    /// `= delete;`
    Deleted,
    /// `;` — a declaration only.
    None,
}

/// `enum class Name : T { ... };`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Enum {
    /// Documentation.
    pub doc: Doc,
    /// Whether it is `enum class` rather than a plain `enum`.
    pub is_scoped: bool,
    /// Name.
    pub name: Ident,
    /// The underlying type, when written.
    pub underlying: Option<Type>,
    /// Enumerators, in written order.
    pub enumerators: Vec<Enumerator>,
}

/// One enumerator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Enumerator {
    /// Documentation.
    pub doc: Doc,
    /// Name.
    pub name: Ident,
    /// `= value`, when written.
    pub value: Option<Expr>,
}

/// `using Name = Type;`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alias {
    /// Documentation.
    pub doc: Doc,
    /// Name.
    pub name: Ident,
    /// What it aliases.
    pub type_: Type,
}

/// A function at namespace scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    /// Documentation.
    pub doc: Doc,
    /// Return type.
    pub returns: Type,
    /// Name.
    pub name: Ident,
    /// Parameters.
    pub params: Vec<Param>,
    /// `inline`.
    pub is_inline: bool,
    /// `static`.
    pub is_static: bool,
    /// `constexpr`.
    pub is_constexpr: bool,
    /// `noexcept`.
    pub is_noexcept: bool,
    /// `[[maybe_unused]]` (C++17 §9.12.8): a function that may
    /// legitimately have no caller, so `-Wunused-function` stays quiet
    /// about it and only about it.
    pub is_maybe_unused: bool,
    /// The body.
    pub body: FunctionBody,
}

/// A variable at namespace scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variable {
    /// Documentation.
    pub doc: Doc,
    /// Type.
    pub type_: Type,
    /// Name.
    pub name: Ident,
    /// Initialiser.
    pub value: Option<Expr>,
    /// `static`.
    pub is_static: bool,
    /// `constexpr`.
    pub is_constexpr: bool,
    /// `inline`.
    pub is_inline: bool,
}

/// One parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    /// Type.
    pub type_: Type,
    /// Name, absent for an unused parameter.
    pub name: Option<Ident>,
    /// A default argument.
    pub default: Option<Expr>,
}

/// `template <..> <decl>`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template {
    /// Documentation.
    pub doc: Doc,
    /// The parameters, empty for an explicit specialisation.
    pub params: Vec<TemplateParam>,
    /// What is templated.
    pub decl: Box<Decl>,
    /// The specialisation arguments, for `template <> struct S<T>`.
    pub specialisation: Vec<Type>,
}

/// One template parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateParam {
    /// The kind of parameter.
    pub kind: TemplateParamKind,
    /// Its name.
    pub name: Ident,
    /// A default argument.
    pub default: Option<Type>,
}

/// Which kind of template parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateParamKind {
    /// `typename T`
    TypeName,
    /// `class T`
    Class,
    /// A non-type parameter of the given type.
    NonType(Box<Type>),
}

/// A C++ type as it will be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    /// A built-in.
    Primitive(Primitive),
    /// A named type with any template arguments.
    Named {
        /// Its path.
        path: Path,
        /// Its template arguments, in written order.
        args: Vec<Type>,
    },
    /// `T *`
    Pointer(Box<Type>),
    /// `T &`
    Ref(Box<Type>),
    /// `T &&`
    RvalueRef(Box<Type>),
    /// `const T`
    Const(Box<Type>),
    /// `T[N]`
    Array {
        /// Element type.
        element: Box<Type>,
        /// Length.
        length: Box<Expr>,
    },
    /// `void`
    Void,
    /// `auto`
    Auto,
}

impl Type {
    /// A named type with no template arguments.
    #[must_use]
    pub fn named(path: Path) -> Self {
        Self::Named {
            path,
            args: Vec::new(),
        }
    }

    /// A named type with template arguments.
    #[must_use]
    pub fn template(path: Path, args: Vec<Self>) -> Self {
        Self::Named { path, args }
    }
}

/// A C++ built-in type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Primitive {
    /// `bool`
    Bool,
    /// `char`
    Char,
    /// `char16_t`
    Char16,
    /// `char32_t`
    Char32,
    /// `wchar_t`
    WChar,
    /// `int8_t`
    Int8,
    /// `int16_t`
    Int16,
    /// `int32_t`
    Int32,
    /// `int64_t`
    Int64,
    /// `uint8_t`
    UInt8,
    /// `uint16_t`
    UInt16,
    /// `uint32_t`
    UInt32,
    /// `uint64_t`
    UInt64,
    /// `float`
    Float,
    /// `double`
    Double,
    /// `long double`
    LongDouble,
    /// `size_t`
    Size,
}

impl Primitive {
    /// Every built-in the tree can hold.
    pub const ALL: [Self; 17] = [
        Self::Bool,
        Self::Char,
        Self::Char16,
        Self::Char32,
        Self::WChar,
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
        Self::LongDouble,
        Self::Size,
    ];

    /// How it is written.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::Char => "char",
            Self::Char16 => "char16_t",
            Self::Char32 => "char32_t",
            Self::WChar => "wchar_t",
            Self::Int8 => "int8_t",
            Self::Int16 => "int16_t",
            Self::Int32 => "int32_t",
            Self::Int64 => "int64_t",
            Self::UInt8 => "uint8_t",
            Self::UInt16 => "uint16_t",
            Self::UInt32 => "uint32_t",
            Self::UInt64 => "uint64_t",
            Self::Float => "float",
            Self::Double => "double",
            Self::LongDouble => "long double",
            Self::Size => "size_t",
        }
    }

    /// The header it comes from, when it needs one.
    #[must_use]
    pub const fn header(self) -> Option<&'static str> {
        match self {
            Self::Int8
            | Self::Int16
            | Self::Int32
            | Self::Int64
            | Self::UInt8
            | Self::UInt16
            | Self::UInt32
            | Self::UInt64 => Some("cstdint"),
            Self::Size => Some("cstddef"),
            Self::Bool
            | Self::Char
            | Self::Char16
            | Self::Char32
            | Self::WChar
            | Self::Float
            | Self::Double
            | Self::LongDouble => None,
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
    /// A declaration with an optional initialiser.
    Declare {
        /// Type.
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
    /// `for (init; cond; step) { .. }`
    ///
    /// Not [`Statement::ForEach`]. A generated codec walks a collection
    /// by index because the index is what the wire needs — an element's
    /// position decides its alignment and, in a `map` written in
    /// ascending key order, which entry is written at all.
    For {
        /// Initialiser.
        init: Box<Statement>,
        /// Condition.
        condition: Expr,
        /// Step.
        step: Expr,
        /// Body.
        body: Block,
    },
    /// `while (cond) { .. }`
    ///
    /// Not a `for` with the two clauses left empty. A decoder walks a
    /// `@mutable` aggregate's headers until the delimiter is used up and
    /// a PL_CDR1 list until its sentinel; neither has a counter.
    While {
        /// Condition.
        condition: Expr,
        /// Body.
        body: Block,
    },
    /// `break;`
    Break,
    /// `for (auto& name : range) { .. }`
    ForEach {
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
        /// The cases, in written order.
        cases: Vec<SwitchCase>,
    },
    /// `throw expr;`
    ///
    /// The Java tree has had one from the start and this one had not,
    /// which held for as long as the only C++ output was data
    /// declarations. A CORBA skeleton's default arm is
    /// `throw CORBA::BAD_OPERATION();` — §8.3 — and a call expression
    /// spelled to look like one would compile and then fall through.
    Throw(Expr),
    /// `try { .. } catch (T& name) { .. } catch (...) { .. }`
    ///
    /// A generated DDS-RPC service turns what an operation throws into the
    /// case of the reply union that carries it (DDS-RPC 1.0 §7.5.1.1.5)
    /// or into a remote exception code (§7.5.2); that needs a handler.
    Try {
        /// The guarded block.
        body: Block,
        /// The handlers, in written order.
        handlers: Vec<CatchClause>,
    },
    /// A comment on its own line.
    Comment(Doc),
}

/// One handler of a [`Statement::Try`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatchClause {
    /// The type caught, taken by const reference; `None` is `catch (...)`.
    pub type_: Option<Type>,
    /// The name the exception is bound to, absent when it is not used.
    pub binding: Option<Ident>,
    /// The handler's body.
    pub body: Block,
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
    /// `f(args)`
    Call {
        /// What is called.
        callee: Box<Expr>,
        /// Arguments.
        args: Vec<Expr>,
    },
    /// `value.name(args)`
    MethodCall {
        /// The receiver.
        receiver: Box<Expr>,
        /// The method.
        name: Ident,
        /// Arguments.
        args: Vec<Expr>,
    },
    /// `a.b`
    Member {
        /// The value.
        value: Box<Expr>,
        /// The member.
        name: Ident,
    },
    /// `a->b`
    Arrow {
        /// The pointer.
        value: Box<Expr>,
        /// The member.
        name: Ident,
    },
    /// `a[i]`
    Index {
        /// The value.
        value: Box<Expr>,
        /// The index.
        index: Box<Expr>,
    },
    /// `T{args}` or `T(args)`.
    Construct {
        /// The type.
        type_: Box<Type>,
        /// Arguments.
        args: Vec<Expr>,
        /// Whether braces are used.
        braced: bool,
    },
    /// `op a`, written as given — `!a`, `-a`, `~a`.
    ///
    /// The tree carries no precedence, so a caller that needs the
    /// operand bracketed writes [`Expr::Paren`] around it.
    ///
    /// Added because there was no form for it and the backend faked
    /// one with `Binary { op, lhs: 0 }`. That reads as `0 - a` for a
    /// negation, which is the same number, and as `0 ~ a` for a
    /// complement, which is not an expression: 21 corpus cases stopped
    /// compiling on `zd_bits & 0 ~ UINT64_C(32897)`.
    Unary {
        /// The operator, as written.
        op: &'static str,
        /// What it applies to.
        value: Box<Expr>,
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
    /// `static_cast<T>(v)` and friends.
    Cast {
        /// Which cast.
        kind: CastKind,
        /// The target type.
        type_: Box<Type>,
        /// The value.
        value: Box<Expr>,
    },
    /// `&a`
    AddressOf(Box<Expr>),
    /// `*a`
    Deref(Box<Expr>),
    /// `{ a, b }`
    InitList(Vec<Expr>),
    /// `sizeof(T)`
    SizeOf(Box<Type>),
    /// `(a)` — parentheses written on purpose.
    ///
    /// The same node the C tree carries and for the same reason: this
    /// tree holds no operator precedence, so a renderer walking it
    /// cannot know where a bracket is needed. A builder that needs one
    /// says so. `Binary { op: "<<", lhs: Binary { op: "+", .. }, .. }`
    /// renders as `a + b << c` without it, which is a different number
    /// and compiles without a word — the C column met that shape in a
    /// `#define` and the same composition is reachable here through a
    /// `constexpr` initialiser or a case label.
    Paren(Box<Expr>),
}

/// Which cast form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CastKind {
    /// `static_cast`
    Static,
    /// `const_cast`
    Const,
    /// `reinterpret_cast`
    Reinterpret,
    /// `dynamic_cast`
    Dynamic,
}

impl CastKind {
    /// Every cast form.
    pub const ALL: [Self; 4] = [Self::Static, Self::Const, Self::Reinterpret, Self::Dynamic];

    /// How it is written.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Static => "static_cast",
            Self::Const => "const_cast",
            Self::Reinterpret => "reinterpret_cast",
            Self::Dynamic => "dynamic_cast",
        }
    }
}

/// The name a C++ keyword collision escapes to.
#[must_use]
pub fn escape_keyword(name: &str) -> String {
    if RESERVED.contains(&name) {
        let mut out = name.to_string();
        out.push('_');
        return out;
    }
    name.to_string()
}

/// The C++ keywords a generated identifier may collide with.
///
/// The C++17 set, which is what the emitters this replaces targeted.
const RESERVED: &[&str] = &[
    "alignas",
    "alignof",
    "and",
    "asm",
    "auto",
    "bitand",
    "bitor",
    "bool",
    "break",
    "case",
    "catch",
    "char",
    "class",
    "compl",
    "concept",
    "const",
    "consteval",
    "constexpr",
    "const_cast",
    "continue",
    "decltype",
    "default",
    "delete",
    "do",
    "double",
    "dynamic_cast",
    "else",
    "enum",
    "explicit",
    "export",
    "extern",
    "false",
    "float",
    "for",
    "friend",
    "goto",
    "if",
    "inline",
    "int",
    "long",
    "mutable",
    "namespace",
    "new",
    "noexcept",
    "not",
    "nullptr",
    "operator",
    "or",
    "private",
    "protected",
    "public",
    "register",
    "reinterpret_cast",
    "requires",
    "return",
    "short",
    "signed",
    "sizeof",
    "static",
    "static_assert",
    "static_cast",
    "struct",
    "switch",
    "template",
    "this",
    "thread_local",
    "throw",
    "true",
    "try",
    "typedef",
    "typeid",
    "typename",
    "union",
    "unsigned",
    "using",
    "virtual",
    "void",
    "volatile",
    "wchar_t",
    "while",
    "xor",
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
    }

    #[test]
    fn access_is_a_section_rather_than_a_word_on_each_member() {
        // The output writes `public:` once above a run; a tree storing
        // access per member would leave the grouping to the renderer.
        let class = Class {
            doc: Doc::none(),
            is_struct: false,
            name: Ident::new("Point"),
            bases: Vec::new(),
            sections: alloc::vec![
                Section {
                    access: Access::Public,
                    members: Vec::new(),
                },
                Section {
                    access: Access::Private,
                    members: Vec::new(),
                },
            ],
        };
        let order: Vec<Access> = class.sections.iter().map(|s| s.access).collect();
        assert_eq!(order, alloc::vec![Access::Public, Access::Private]);
    }

    #[test]
    fn a_keyword_escapes_with_a_trailing_underscore() {
        assert_eq!(escape_keyword("class"), "class_");
        assert_eq!(escape_keyword("operator"), "operator_");
        assert_eq!(escape_keyword("value"), "value");
    }

    #[test]
    fn a_defaulted_constructor_is_its_own_body_form() {
        // `= default` is not an empty block: the two compile
        // differently, and the tree has to keep them apart.
        assert_ne!(
            FunctionBody::Defaulted,
            FunctionBody::Block(Block::default())
        );
        assert_ne!(FunctionBody::Deleted, FunctionBody::None);
    }
}
