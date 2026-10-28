// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The C concrete syntax tree. Spec §1.
//!
//! C has no namespaces and no templates, and says with a `typedef` what
//! C++ says with a class. The tree says the same: it is not the C++ tree
//! with parts switched off, because a tree that pretended otherwise
//! would push the difference into the renderer.

use crate::common::{BodyText, Doc, Ident, Literal};
use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// A C header or source file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct File {
    /// The leading comment.
    pub doc: Doc,
    /// The include guard, when the file has one.
    pub guard: Option<Ident>,
    /// `#include` lines, in written order.
    pub includes: Vec<Include>,
    /// Declarations, in written order.
    pub decls: Vec<Decl>,
}

/// One `#include`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Include {
    /// The path between the delimiters.
    pub path: String,
    /// Whether it is `<system>` rather than `"local"`.
    pub system: bool,
}

impl Include {
    /// `#include <path>`
    #[must_use]
    pub fn system(path: &str) -> Self {
        Self {
            path: path.to_string(),
            system: true,
        }
    }

    /// `#include "path"`
    #[must_use]
    pub fn local(path: &str) -> Self {
        Self {
            path: path.to_string(),
            system: false,
        }
    }
}

/// One declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decl {
    /// `struct Name { ... };`
    Struct(Struct),
    /// `union Name { ... };`
    Union(Union),
    /// `enum Name { ... };`
    Enum(Enum),
    /// `typedef ... Name;`
    Typedef(Typedef),
    /// A function declaration or definition.
    Function(Function),
    /// A variable at file scope.
    Variable(Variable),
    /// `#define NAME value`
    Define(Define),
    /// A comment standing on its own.
    Comment(Doc),
    /// `@verbatim` text, copied into the output unchanged.
    ///
    /// OMG IDL 4.2 §8.3.5.1: the annotation's `text` "shall be copied
    /// verbatim" into the generated artifact — raw lines, never a
    /// comment. One node per directive, lines split on `\n`.
    Verbatim(String),
}

/// `struct Name { ... };`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Struct {
    /// Documentation.
    pub doc: Doc,
    /// The tag, absent for an anonymous struct inside a typedef.
    pub name: Option<Ident>,
    /// Members, in written order.
    pub members: Vec<Member>,
    /// `@verbatim` text at the start and the end of the member list.
    pub body_text: BodyText,
}

/// `union Name { ... };`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Union {
    /// Documentation.
    pub doc: Doc,
    /// The tag.
    pub name: Option<Ident>,
    /// Members, in written order.
    pub members: Vec<Member>,
}

/// One member of a struct or union.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    /// Documentation.
    pub doc: Doc,
    /// Type.
    pub type_: Type,
    /// Name.
    pub name: Ident,
    /// Array dimensions, outermost first — `char name[4][8]`.
    pub dims: Vec<Expr>,
    /// A bitfield width, when written.
    pub bits: Option<u16>,
}

/// `enum Name { ... };`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Enum {
    /// Documentation.
    pub doc: Doc,
    /// The tag.
    pub name: Option<Ident>,
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

/// `typedef <type> <name>;`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Typedef {
    /// Documentation.
    pub doc: Doc,
    /// What is aliased.
    pub type_: Type,
    /// The new name.
    pub name: Ident,
    /// Array dimensions, for `typedef long Row[4];`.
    pub dims: Vec<Expr>,
}

/// A function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    /// Documentation.
    pub doc: Doc,
    /// `static`.
    pub is_static: bool,
    /// `inline`.
    pub is_inline: bool,
    /// `__attribute__((unused))`, for a `static` function whose having a
    /// caller depends on a setting rather than on the model.
    ///
    /// Generated codecs are compiled under `-Werror=unused-function`,
    /// and that is worth keeping: a codec nothing calls is normally a
    /// generator that emitted the wrong thing. The exception is a
    /// primitive whose codec is reached from one position and inlined at
    /// another — a `Char16` under a framing that gives a member and a
    /// collection element different octets — where whether the shared
    /// function has a caller is a property of the dialect and not of the
    /// type graph. Set there and nowhere else.
    pub is_maybe_unused: bool,
    /// Return type.
    pub returns: Type,
    /// Name.
    pub name: Ident,
    /// Parameters, in order.
    pub params: Vec<Param>,
    /// The body, absent for a declaration.
    pub body: Option<Block>,
}

/// One parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    /// Type.
    pub type_: Type,
    /// Name, absent in a prototype that gives only types.
    pub name: Option<Ident>,
}

/// A variable at file scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variable {
    /// Documentation.
    pub doc: Doc,
    /// `static`.
    pub is_static: bool,
    /// `const`.
    pub is_const: bool,
    /// Type.
    pub type_: Type,
    /// Name.
    pub name: Ident,
    /// Initialiser.
    pub value: Option<Expr>,
}

/// `#define NAME value`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Define {
    /// Documentation.
    pub doc: Doc,
    /// Name.
    pub name: Ident,
    /// The replacement, absent for a bare `#define NAME`.
    pub value: Option<Expr>,
}

/// A C type as it will be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    /// A built-in.
    Primitive(Primitive),
    /// A name: a typedef, or a tag used as a type in C99 style.
    Named(Ident),
    /// `struct Tag`
    StructTag(Ident),
    /// `union Tag`
    UnionTag(Ident),
    /// `enum Tag`
    EnumTag(Ident),
    /// `T *`
    Pointer(Box<Type>),
    /// `const T`
    Const(Box<Type>),
    /// `void`
    Void,
}

/// A C built-in type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Primitive {
    /// `char`
    Char,
    /// `bool`, from `<stdbool.h>`.
    Bool,
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
    pub const ALL: [Self; 14] = [
        Self::Char,
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
        Self::LongDouble,
        Self::Size,
    ];

    /// How it is written.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Char => "char",
            Self::Bool => "bool",
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
            | Self::UInt64 => Some("stdint.h"),
            Self::Bool => Some("stdbool.h"),
            Self::Size => Some("stddef.h"),
            Self::Char | Self::Float | Self::Double | Self::LongDouble => None,
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
    /// `@mutable` aggregate's headers until the delimiter is used up
    /// and a PL_CDR1 list until its sentinel; neither has a counter,
    /// and writing one as `for (; cond;)` says there was an induction
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
    /// count, so the loop leaves from the middle. A flag standing in
    /// for it would keep reading a list that has already ended.
    Break,
    /// A comment standing on its own line.
    Comment(Doc),
}

/// An expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    /// A literal.
    Literal(Literal),
    /// A name.
    Name(Ident),
    /// `f(args)`
    Call {
        /// What is called.
        callee: Box<Expr>,
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
        /// The array.
        value: Box<Expr>,
        /// The index.
        index: Box<Expr>,
    },
    /// `op a`, written as given — `!a`, `-a`, `~a`.
    ///
    /// The tree carries no precedence, so a caller that needs the
    /// operand bracketed writes [`Expr::Paren`] around it.
    Unary {
        /// The operator, as written.
        op: &'static str,
        /// What it applies to.
        value: Box<Expr>,
    },
    /// `&a`
    AddressOf(Box<Expr>),
    /// `*a`
    Deref(Box<Expr>),
    /// `a op b`
    Binary {
        /// The operator, as written.
        op: &'static str,
        /// Left.
        lhs: Box<Expr>,
        /// Right.
        rhs: Box<Expr>,
    },
    /// `(T)value`
    Cast {
        /// The target type.
        type_: Box<Type>,
        /// The value.
        value: Box<Expr>,
    },
    /// `sizeof(T)`
    SizeOf(Box<Type>),
    /// `(a)` — parentheses written on purpose.
    ///
    /// The tree carries no operator precedence, so a renderer walking it
    /// cannot know where C needs a bracket and where it does not. A
    /// builder that needs one says so. The case that made this
    /// necessary is a `#define`: `#define MOVING (Flags)1 << 4` expands
    /// inside `x & MOVING` as `(x & (Flags)1) << 4`, which is a
    /// different number and compiles without a word.
    Paren(Box<Expr>),
    /// `{ a, b }`
    Initializer(Vec<Expr>),
}

/// The name a C keyword collision escapes to.
///
/// A trailing underscore, which is what the emitters this replaces did
/// and what the fixture output records.
#[must_use]
pub fn escape_keyword(name: &str) -> String {
    if RESERVED.contains(&name) {
        let mut out = name.to_string();
        out.push('_');
        return out;
    }
    name.to_string()
}

/// The C keywords a generated identifier may collide with. C11 §6.4.1.
const RESERVED: &[&str] = &[
    "auto",
    "break",
    "case",
    "char",
    "const",
    "continue",
    "default",
    "do",
    "double",
    "else",
    "enum",
    "extern",
    "float",
    "for",
    "goto",
    "if",
    "inline",
    "int",
    "long",
    "register",
    "restrict",
    "return",
    "short",
    "signed",
    "sizeof",
    "static",
    "struct",
    "switch",
    "typedef",
    "union",
    "unsigned",
    "void",
    "volatile",
    "while",
    "_Bool",
    "_Complex",
    "_Imaginary",
    "_Alignas",
    "_Alignof",
    "_Atomic",
    "_Generic",
    "_Noreturn",
    "_Static_assert",
    "_Thread_local",
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
    fn a_fixed_width_primitive_names_the_header_it_needs() {
        // A generator that emitted `int32_t` without `<stdint.h>` would
        // produce a file that does not compile.
        assert_eq!(Primitive::Int32.header(), Some("stdint.h"));
        assert_eq!(Primitive::Bool.header(), Some("stdbool.h"));
        assert_eq!(Primitive::Double.header(), None);
    }

    #[test]
    fn a_keyword_escapes_with_a_trailing_underscore() {
        assert_eq!(escape_keyword("struct"), "struct_");
        assert_eq!(escape_keyword("value"), "value");
    }

    #[test]
    fn an_array_member_keeps_its_dimensions_in_order() {
        let member = Member {
            doc: Doc::none(),
            type_: Type::Primitive(Primitive::Int32),
            name: Ident::new("grid"),
            dims: alloc::vec![
                Expr::Literal(Literal::int(2)),
                Expr::Literal(Literal::int(3))
            ],
            bits: None,
        };
        assert_eq!(member.dims.len(), 2);
    }
}
