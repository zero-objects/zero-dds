// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Java concrete syntax tree. Spec §1.
//!
//! Java has annotations, type parameters and a package per file. It has
//! no free functions and no header/source split, so the tree has neither.

use crate::common::{Doc, Ident, Literal, Path};
use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// A Java source file, which holds one public type.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct File {
    /// The leading comment.
    pub doc: Doc,
    /// The package, absent for the default package.
    pub package: Option<Path>,
    /// `import` lines, in written order.
    pub imports: Vec<Import>,
    /// Type declarations; the first is the file's public type.
    pub types: Vec<TypeDecl>,
}

/// One `import`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Import {
    /// What is imported.
    pub path: Path,
    /// Whether it is `import static`.
    pub static_: bool,
    /// Whether it ends in `.*`.
    pub wildcard: bool,
}

/// One type declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeDecl {
    /// `class Name { ... }`
    Class(Class),
    /// `interface Name { ... }`
    Interface(Interface),
    /// `enum Name { ... }`
    Enum(Enum),
    /// `record Name(..) { ... }`
    Record(Record),
    /// `@interface Name { ... }` — an annotation type.
    AnnotationType(AnnotationType),
    /// `@verbatim` text at file level, copied into the output unchanged.
    ///
    /// OMG IDL 4.2 §8.3.5.1: the annotation's `text` "shall be copied
    /// verbatim" into the generated artifact — raw lines, never a
    /// comment. One node per directive, lines split on `\n`.
    Verbatim(String),
}

/// `class Name<T> extends B implements I { ... }`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Class {
    /// Documentation.
    pub doc: Doc,
    /// Annotations, in written order.
    pub annotations: Vec<Annotation>,
    /// Modifiers, in written order.
    pub modifiers: Vec<Modifier>,
    /// Name.
    pub name: Ident,
    /// Type parameters.
    pub generics: Vec<TypeParam>,
    /// The superclass.
    pub extends: Option<Type>,
    /// Interfaces implemented.
    pub implements: Vec<Type>,
    /// Members, in written order.
    pub members: Vec<Member>,
}

/// `interface Name { ... }`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interface {
    /// Documentation.
    pub doc: Doc,
    /// Annotations.
    pub annotations: Vec<Annotation>,
    /// Modifiers.
    pub modifiers: Vec<Modifier>,
    /// Name.
    pub name: Ident,
    /// Type parameters.
    pub generics: Vec<TypeParam>,
    /// Interfaces extended.
    pub extends: Vec<Type>,
    /// Members.
    pub members: Vec<Member>,
}

/// `enum Name { A, B; ... }`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Enum {
    /// Documentation.
    pub doc: Doc,
    /// Annotations.
    pub annotations: Vec<Annotation>,
    /// Modifiers.
    pub modifiers: Vec<Modifier>,
    /// Name.
    pub name: Ident,
    /// Interfaces implemented.
    pub implements: Vec<Type>,
    /// The constants, in written order.
    pub constants: Vec<EnumConstant>,
    /// Members after the constants.
    pub members: Vec<Member>,
}

/// One enum constant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumConstant {
    /// Documentation.
    pub doc: Doc,
    /// Annotations.
    pub annotations: Vec<Annotation>,
    /// Name.
    pub name: Ident,
    /// Constructor arguments.
    pub args: Vec<Expr>,
    /// A class body, for a constant that overrides something.
    pub body: Vec<Member>,
}

/// `record Name(components) { ... }`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    /// Documentation.
    pub doc: Doc,
    /// Annotations.
    pub annotations: Vec<Annotation>,
    /// Modifiers.
    pub modifiers: Vec<Modifier>,
    /// Name.
    pub name: Ident,
    /// Type parameters.
    pub generics: Vec<TypeParam>,
    /// Components, in written order.
    pub components: Vec<Param>,
    /// Interfaces implemented.
    pub implements: Vec<Type>,
    /// Members.
    pub members: Vec<Member>,
}

/// `@interface Name { ... }`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnotationType {
    /// Documentation.
    pub doc: Doc,
    /// Annotations on it, such as `@Retention`.
    pub annotations: Vec<Annotation>,
    /// Modifiers.
    pub modifiers: Vec<Modifier>,
    /// Name.
    pub name: Ident,
    /// Its elements.
    pub elements: Vec<AnnotationElement>,
}

/// One element of an annotation type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnotationElement {
    /// Documentation.
    pub doc: Doc,
    /// Type.
    pub type_: Type,
    /// Name.
    pub name: Ident,
    /// `default value`, when written.
    pub default: Option<Expr>,
}

/// One member of a type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Member {
    /// A field.
    Field(Field),
    /// A method.
    Method(Method),
    /// A constructor.
    Constructor(Constructor),
    /// A nested type.
    Nested(Box<TypeDecl>),
    /// A static or instance initialiser block.
    Initialiser {
        /// Whether it is `static`.
        static_: bool,
        /// The block.
        body: Block,
    },
    /// A comment on its own line.
    Comment(Doc),
    /// `@verbatim` text inside the class body, copied unchanged.
    ///
    /// OMG IDL 4.2 §8.3.5.1 `begin-declaration` / `end-declaration`:
    /// the text lands inside the annotated declaration's scope.
    Verbatim(String),
}

/// A field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// Documentation.
    pub doc: Doc,
    /// Annotations.
    pub annotations: Vec<Annotation>,
    /// Modifiers.
    pub modifiers: Vec<Modifier>,
    /// Type.
    pub type_: Type,
    /// Name.
    pub name: Ident,
    /// Initialiser.
    pub value: Option<Expr>,
}

/// A method.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Method {
    /// Documentation.
    pub doc: Doc,
    /// Annotations.
    pub annotations: Vec<Annotation>,
    /// Modifiers.
    pub modifiers: Vec<Modifier>,
    /// Type parameters.
    pub generics: Vec<TypeParam>,
    /// Return type.
    pub returns: Type,
    /// Name.
    pub name: Ident,
    /// Parameters.
    pub params: Vec<Param>,
    /// `throws`, in written order.
    pub throws: Vec<Type>,
    /// The body, absent for an abstract or interface method.
    pub body: Option<Block>,
}

/// A constructor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Constructor {
    /// Documentation.
    pub doc: Doc,
    /// Annotations.
    pub annotations: Vec<Annotation>,
    /// Modifiers.
    pub modifiers: Vec<Modifier>,
    /// The class's name.
    pub name: Ident,
    /// Parameters.
    pub params: Vec<Param>,
    /// `throws`.
    pub throws: Vec<Type>,
    /// The body.
    pub body: Block,
}

/// One parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    /// Annotations.
    pub annotations: Vec<Annotation>,
    /// Whether it is `final`.
    pub final_: bool,
    /// Type.
    pub type_: Type,
    /// Name.
    pub name: Ident,
    /// Whether it is a varargs parameter.
    pub varargs: bool,
}

/// A modifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Modifier {
    /// `public`
    Public,
    /// `protected`
    Protected,
    /// `private`
    Private,
    /// `static`
    Static,
    /// `final`
    Final,
    /// `abstract`
    Abstract,
    /// `synchronized`
    Synchronized,
    /// `transient`
    Transient,
    /// `volatile`
    Volatile,
    /// `native`
    Native,
    /// `strictfp`
    Strictfp,
    /// `default`, on an interface method.
    Default,
    /// `sealed`
    Sealed,
}

impl Modifier {
    /// Every modifier the tree can hold.
    pub const ALL: [Self; 13] = [
        Self::Public,
        Self::Protected,
        Self::Private,
        Self::Static,
        Self::Final,
        Self::Abstract,
        Self::Synchronized,
        Self::Transient,
        Self::Volatile,
        Self::Native,
        Self::Strictfp,
        Self::Default,
        Self::Sealed,
    ];

    /// How it is written.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Protected => "protected",
            Self::Private => "private",
            Self::Static => "static",
            Self::Final => "final",
            Self::Abstract => "abstract",
            Self::Synchronized => "synchronized",
            Self::Transient => "transient",
            Self::Volatile => "volatile",
            Self::Native => "native",
            Self::Strictfp => "strictfp",
            Self::Default => "default",
            Self::Sealed => "sealed",
        }
    }
}

/// One annotation use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Annotation {
    /// Its name.
    pub path: Path,
    /// Its arguments, in written order.
    pub args: Vec<AnnotationArg>,
}

/// One argument of an annotation use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnnotationArg {
    /// The single unnamed argument: `@Ann(value)`.
    Value(Expr),
    /// `name = value`
    Pair {
        /// The name.
        name: Ident,
        /// The value.
        value: Expr,
    },
}

/// One type parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeParam {
    /// Name.
    pub name: Ident,
    /// Bounds, in written order.
    pub bounds: Vec<Type>,
}

/// A Java type as it will be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    /// A built-in.
    Primitive(Primitive),
    /// A named type with any type arguments.
    Named {
        /// Its path.
        path: Path,
        /// Its type arguments, in written order.
        args: Vec<Type>,
    },
    /// `T[]`
    Array(Box<Type>),
    /// `?`, `? extends T`, `? super T`.
    Wildcard {
        /// An upper bound.
        extends: Option<Box<Type>>,
        /// A lower bound.
        super_: Option<Box<Type>>,
    },
    /// `void`
    Void,
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
}

/// A Java primitive type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Primitive {
    /// `boolean`
    Boolean,
    /// `byte`
    Byte,
    /// `char`
    Char,
    /// `short`
    Short,
    /// `int`
    Int,
    /// `long`
    Long,
    /// `float`
    Float,
    /// `double`
    Double,
}

impl Primitive {
    /// Every primitive Java has.
    pub const ALL: [Self; 8] = [
        Self::Boolean,
        Self::Byte,
        Self::Char,
        Self::Short,
        Self::Int,
        Self::Long,
        Self::Float,
        Self::Double,
    ];

    /// How it is written.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Boolean => "boolean",
            Self::Byte => "byte",
            Self::Char => "char",
            Self::Short => "short",
            Self::Int => "int",
            Self::Long => "long",
            Self::Float => "float",
            Self::Double => "double",
        }
    }

    /// The wrapper class, for a type argument that cannot be primitive.
    #[must_use]
    pub const fn boxed(self) -> &'static str {
        match self {
            Self::Boolean => "Boolean",
            Self::Byte => "Byte",
            Self::Char => "Character",
            Self::Short => "Short",
            Self::Int => "Integer",
            Self::Long => "Long",
            Self::Float => "Float",
            Self::Double => "Double",
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
    /// A local declaration.
    Declare {
        /// Whether it is `final`.
        final_: bool,
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
    /// `for (T name : range) { .. }`
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
    /// `for (init; cond; step) { .. }`
    ///
    /// Not [`Statement::ForEach`]. A generated codec walks a collection
    /// by index because the index is what the wire needs — an element's
    /// position decides its alignment and, in a `map` written in
    /// ascending key order, which entry is written at all. A `for-each`
    /// hands the element and hides the position.
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
    /// a PL_CDR1 list until its sentinel; neither has a counter, and
    /// writing one as `for (; cond;)` says there was an induction
    /// variable somebody forgot.
    While {
        /// Condition.
        condition: Expr,
        /// Body.
        body: Block,
    },
    /// `break;`
    ///
    /// A PL_CDR1 parameter list ends at `PID_LIST_END` and not at a
    /// count, so the loop leaves from the middle.
    Break,
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
    /// `receiver.name(args)`
    MethodCall {
        /// The receiver, absent for an unqualified call.
        receiver: Option<Box<Expr>>,
        /// The method.
        name: Ident,
        /// Arguments.
        args: Vec<Expr>,
    },
    /// `value.name`
    Field {
        /// The value.
        value: Box<Expr>,
        /// The field.
        name: Ident,
    },
    /// `a[i]`
    Index {
        /// The array.
        value: Box<Expr>,
        /// The index.
        index: Box<Expr>,
    },
    /// `new T(args)`
    New {
        /// The type.
        type_: Box<Type>,
        /// Arguments.
        args: Vec<Expr>,
    },
    /// `new T[]{ a, b }`, `new T[n]`, `new T[n][m]`, `new T[n][]`.
    NewArray {
        /// The element type, which may itself be an array.
        ///
        /// Its own `[]`s are written *after* the sized dimensions:
        /// `new int[n][]` allocates `n` rows of `int[]`, and
        /// `new int[][n]` is not a form Java has. A sequence of a
        /// fixed-size array reaches exactly that shape.
        element: Box<Type>,
        /// One per sized dimension, outermost first. Empty asks for the
        /// initialiser form.
        lengths: Vec<Expr>,
        /// The elements, for the initialiser form.
        elements: Vec<Expr>,
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
    /// `(value)`, written because the writer meant it.
    ///
    /// The tree carries no precedence and the renderer adds no
    /// brackets, so a caller that needs one says so. Java's `&` binds
    /// *looser* than `!=`, which makes `bits & mask != 0` a type error
    /// where C reads it as a comparison — and looser than `^`, which
    /// makes `bits & a ^ b` a different number with nothing to say so.
    Paren(Box<Expr>),
    /// `(T) value`
    Cast {
        /// The target type.
        type_: Box<Type>,
        /// The value.
        value: Box<Expr>,
    },
    /// `value instanceof T`
    InstanceOf {
        /// The value.
        value: Box<Expr>,
        /// The type.
        type_: Box<Type>,
    },
    /// `cond ? a : b`
    Ternary {
        /// The condition.
        condition: Box<Expr>,
        /// The then-value.
        then: Box<Expr>,
        /// The else-value.
        otherwise: Box<Expr>,
    },
    /// `this`
    This,
    /// A lambda: `(a, b) -> body`.
    Lambda {
        /// Parameter names.
        params: Vec<Ident>,
        /// The body as an expression.
        body: Box<Expr>,
    },
    /// An annotation used as a value: an element of an annotation type
    /// that is itself an annotation, or of an array of them.
    Annotation(Annotation),
    /// `{ a, b }`, the array initialiser of an annotation element.
    ArrayInit(Vec<Expr>),
}

/// The name a Java keyword collision escapes to.
#[must_use]
pub fn escape_keyword(name: &str) -> String {
    if RESERVED.contains(&name) {
        let mut out = name.to_string();
        out.push('_');
        return out;
    }
    name.to_string()
}

/// The Java keywords a generated identifier may collide with.
/// JLS 17 §3.9, plus the three literals that are not identifiers.
const RESERVED: &[&str] = &[
    "abstract",
    "assert",
    "boolean",
    "break",
    "byte",
    "case",
    "catch",
    "char",
    "class",
    "const",
    "continue",
    "default",
    "do",
    "double",
    "else",
    "enum",
    "extends",
    "final",
    "finally",
    "float",
    "for",
    "goto",
    "if",
    "implements",
    "import",
    "instanceof",
    "int",
    "interface",
    "long",
    "native",
    "new",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "short",
    "static",
    "strictfp",
    "super",
    "switch",
    "synchronized",
    "this",
    "throw",
    "throws",
    "transient",
    "try",
    "void",
    "volatile",
    "while",
    "true",
    "false",
    "null",
];

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn every_primitive_writes_its_own_spelling_and_has_a_wrapper() {
        let mut seen = Vec::new();
        let mut boxed = Vec::new();
        for primitive in Primitive::ALL {
            assert!(!seen.contains(&primitive.as_str()));
            seen.push(primitive.as_str());
            assert!(!boxed.contains(&primitive.boxed()));
            boxed.push(primitive.boxed());
        }
    }

    #[test]
    fn char_boxes_to_character_not_char() {
        // The one wrapper whose name is not the primitive capitalised;
        // getting it wrong produces a type that does not exist.
        assert_eq!(Primitive::Char.boxed(), "Character");
        assert_eq!(Primitive::Int.boxed(), "Integer");
    }

    #[test]
    fn a_keyword_escapes_with_a_trailing_underscore() {
        assert_eq!(escape_keyword("class"), "class_");
        assert_eq!(escape_keyword("null"), "null_");
        assert_eq!(escape_keyword("value"), "value");
    }

    #[test]
    fn modifiers_keep_the_order_they_were_written_in() {
        // `public static final` is the conventional order and the one
        // the output has; the tree records it rather than sorting.
        let field = Field {
            doc: Doc::none(),
            annotations: Vec::new(),
            modifiers: alloc::vec![Modifier::Public, Modifier::Static, Modifier::Final],
            type_: Type::Primitive(Primitive::Int),
            name: Ident::new("MAX"),
            value: None,
        };
        let written: Vec<&str> = field.modifiers.iter().map(|m| m.as_str()).collect();
        assert_eq!(written, alloc::vec!["public", "static", "final"]);
    }
}
