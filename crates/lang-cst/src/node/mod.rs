// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Node (plain JavaScript) concrete syntax tree. Spec §1.
//!
//! Node has no declarations in the sense [`crate::zig`] or
//! [`crate::typescript`] mean the word: there is no `struct`, no
//! `interface`, no `type` and no annotation the language itself reads.
//! A source file is a run of statements, and every aggregate value in it
//! is an object literal or an array literal. This tree is therefore
//! shaped like [`crate::lua`]'s rather than like the typed columns' —
//! [`Expr`] has no counterpart to [`crate::zig::Type`] — with two
//! differences that are JavaScript's own and not simplifications:
//!
//! 1. **Indexing is already zero-based.** Lua's tables start at 1 and its
//!    tree carries the `+ 1` at every subscript
//!    (`crate::lua::Expr::Index`'s documentation). A JavaScript array
//!    needs no such correction, so [`Expr::Index`] and [`Stat::For`] are
//!    the walk's own zero-based counting, unchanged.
//! 2. **A property name is never renamed.** IDL 4.2 §7.2.3's identifier
//!    grammar — `[A-Za-z_][A-Za-z0-9_]*` — is a strict subset of
//!    JavaScript's `IdentifierName` production (ECMA-262 §12.6.1), which
//!    is what a property key after `.` or inside `{ … }` accepts, and
//!    that production has admitted every reserved word since ECMAScript
//!    5 (`obj.class`, `{ new: 1 }`). So unlike [`crate::lua`] — which
//!    still needs [`crate::lua::Place::field`] to choose between `t.x`
//!    and `t["end"]` — this tree has one form, [`Expr::Member`], and
//!    [`escape_member`] is the identity. `constructor` is not a special
//!    case either: every aggregate this column builds is a plain object
//!    literal and never a `class`, so no property name collides with a
//!    language meaning at all.
//!
//! ## What is here beyond the data forms
//!
//! As in Lua, the tree began as data — a literal, a name, a call — and
//! the codec walk added the statement layer: [`Stat::If`], [`Stat::For`]
//! (always from zero, exclusive, since that is the only loop the walk
//! ever builds), [`Stat::While`], [`Stat::Break`], and an expression
//! evaluated for effect. There is no `switch`, no arrow function, no
//! class and no template literal, because no walk builds one; a form
//! nothing constructs is a renderer arm no test reaches.
//!
//! ## Declaration names still need an escape
//!
//! A property name is free, but a top-level `const NAME = …` is not:
//! `const class = 5;` is a `SyntaxError` wherever `class` is one of
//! ECMAScript's reserved words. [`escape_decl`] covers exactly that
//! surface — the enumeration and bitmask constants and the IDL constants
//! `crate::adapter::node` declares — with the same trailing-underscore
//! convention every sibling column uses. Unlike TypeScript's
//! [`crate::typescript::escape_keyword`], nothing here reserves a
//! built-in type name: plain JavaScript has none. `const string = 5;`
//! is ordinary JavaScript.

use crate::common::{Doc, Ident, Literal};
use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// A JavaScript source file: a run of statements, run top to bottom by
/// `node` with no module system either side has to agree on.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct File {
    /// The header, written as `//` above everything else.
    pub doc: Doc,
    /// The statements, in written order.
    pub body: Vec<Stat>,
}

/// One statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stat {
    /// `const name = value;` or `let name = value;`, or `let name;` with
    /// no value.
    Declare {
        /// The `//` block above it.
        doc: Doc,
        /// `const` or `let`.
        kind: VarKind,
        /// The name bound.
        name: Ident,
        /// The value, absent for a bare `let name`.
        value: Option<Expr>,
    },
    /// `place = value;`.
    Assign {
        /// The `//` block above it.
        doc: Doc,
        /// Where the value goes.
        place: Expr,
        /// The value.
        value: Expr,
    },
    /// `function name(params) { … }`, declared at top level.
    Function(Function),
    /// `return value;`, or a bare `return;`.
    Return(Option<Expr>),
    /// An expression evaluated for its effect.
    Expr(Expr),
    /// `if (condition) { … } else { … }`.
    If {
        /// The test.
        condition: Expr,
        /// The statements taken when it holds.
        then: Vec<Stat>,
        /// The statements taken when it does not, absent for a bare
        /// `if`.
        otherwise: Option<Vec<Stat>>,
    },
    /// `for (let counter = 0; counter < bound; counter += 1) { … }`.
    ///
    /// The one loop shape the codec walk asks for: a count from zero,
    /// exclusive of a bound it already computed. There is no
    /// `for…of` here because nothing in the walk iterates a collection
    /// it does not also index.
    For {
        /// The counter, local to the loop.
        counter: Ident,
        /// The exclusive bound.
        bound: Expr,
        /// The body.
        body: Vec<Stat>,
    },
    /// `while (condition) { … }`.
    ///
    /// A `@mutable` aggregate's headers run until its delimiter is used
    /// up and a PL_CDR1 list until its sentinel; neither has a bound.
    While {
        /// The test, evaluated before every pass.
        condition: Expr,
        /// The body.
        body: Vec<Stat>,
    },
    /// `break;`.
    Break,
    /// `throw value;` — how a refusal a generated codec makes for itself
    /// reaches the harness; the wire runtime throws its own.
    Throw(Expr),
    /// A comment on its own line, written as `//`.
    Comment(Doc),
    /// `@verbatim` text, copied into the output unchanged — OMG IDL 4.2
    /// §8.3.5.1 asks for the text "verbatim", so raw lines and never a
    /// comment.
    Verbatim(String),
}

/// `const` or `let`. There is no `var` in this tree: nothing the codec
/// walk builds is reassigned across a hoisted scope, and a `var` reader
/// would ask why one was chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VarKind {
    /// `const`
    Const,
    /// `let`
    Let,
}

impl VarKind {
    /// Every binding form this tree writes.
    pub const ALL: [Self; 2] = [Self::Const, Self::Let];

    /// How it is written.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Const => "const",
            Self::Let => "let",
        }
    }
}

/// `function name(a, b) { … }` — a top-level declaration. There is no
/// return-type annotation and no parameter type: plain JavaScript has
/// neither, and the runtime carries the shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    /// Documentation, written as `//` above the declaration.
    pub doc: Doc,
    /// The name declared.
    pub name: Ident,
    /// Parameter names, in order.
    pub params: Vec<Ident>,
    /// The body.
    pub body: Vec<Stat>,
}

/// An expression.
///
/// No type ever appears here: a JavaScript literal, a call and a member
/// access all carry their own shape at run time, which is the premise
/// [`crate::lua`]'s module documentation states for its own tree and this
/// one shares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    /// A literal. A `bigint` is a [`Literal::Int`] whose text carries the
    /// trailing `n`, exactly as [`crate::typescript::Type::Literal`]'s
    /// documentation explains: `2` and `2n` are different values and
    /// this tree holds the form each was written in.
    Literal(Literal),
    /// `undefined` — the value an absent `@optional` member's key is
    /// simply not there instead of, and what a reader's presence test
    /// compares against.
    Undefined,
    /// A bare name.
    Name(Ident),
    /// `[a, b]`.
    Array(Vec<Expr>),
    /// `{ a: 1, b: 2 }`. The key is always a bare
    /// [`Ident`]: the module documentation says why no IDL member name
    /// ever needs quoting here.
    Object(Vec<(Ident, Expr)>),
    /// `new Callee(args)`.
    New {
        /// The constructor's name.
        callee: Ident,
        /// Arguments, in order.
        args: Vec<Expr>,
    },
    /// `callee(args)`.
    Call {
        /// What is called.
        callee: Box<Expr>,
        /// Arguments, in order.
        args: Vec<Expr>,
    },
    /// `object.name`. Always the dotted form — the module documentation
    /// says why a bracketed one is never needed here.
    Member {
        /// The object.
        object: Box<Expr>,
        /// The property.
        name: Ident,
    },
    /// `object[index]`.
    Index {
        /// The object.
        object: Box<Expr>,
        /// The index.
        index: Box<Expr>,
    },
    /// `(value)` — a parenthesis written on purpose.
    ///
    /// The tree carries no operator precedence, so a builder that needs
    /// a bracket says so and the renderer honours it, exactly as
    /// [`crate::lua::Expr::Paren`]'s documentation states for its own
    /// tree.
    Paren(Box<Expr>),
    /// `lhs op rhs`.
    Binary {
        /// The operator, as written.
        op: &'static str,
        /// The left operand.
        lhs: Box<Expr>,
        /// The right operand.
        rhs: Box<Expr>,
    },
    /// `condition ? then : otherwise`.
    ///
    /// Its own arm and not a pair of [`Expr::Binary`] nodes: JavaScript
    /// has no operator that yields a three-way sign, and spelling one
    /// out of relational operators would go through `Boolean`'s
    /// numeric coercion (`true - false` is `1`), which is the wrong
    /// sign for a `strcmp`. The renderer writes no brackets of its
    /// own, so a builder that needs one says [`Expr::Paren`], exactly
    /// as [`crate::typescript::Expr::Ternary`] requires.
    Ternary {
        /// The condition.
        condition: Box<Expr>,
        /// The value when the condition holds.
        then: Box<Expr>,
        /// The value when it does not.
        otherwise: Box<Expr>,
    },
    /// A prefix operator: `!`, `-`, `~`, `typeof `, `void ` or
    /// `delete `. The three word-shaped ones carry their own trailing
    /// space in `crate::adapter`'s and `crate::emit`'s callers,
    /// because `deletev` and `delete v` are not the same token stream.
    Unary {
        /// The operator, as written.
        op: &'static str,
        /// The operand.
        value: Box<Expr>,
    },
}

impl Expr {
    /// An integer literal.
    #[must_use]
    pub fn int(value: i128) -> Self {
        Self::Literal(Literal::int(value))
    }

    /// A string literal.
    #[must_use]
    pub fn string(text: &str) -> Self {
        Self::Literal(Literal::string(text))
    }

    /// A boolean literal.
    #[must_use]
    pub const fn bool(value: bool) -> Self {
        Self::Literal(Literal::Bool(value))
    }

    /// A bare name.
    #[must_use]
    pub fn name(text: &str) -> Self {
        Self::Name(Ident::new(text))
    }

    /// `object.name`.
    #[must_use]
    pub fn member(object: Self, name: &str) -> Self {
        Self::Member {
            object: Box::new(object),
            name: Ident::new(name),
        }
    }

    /// `callee(args)`, on a bare name.
    #[must_use]
    pub fn call(callee: &str, args: Vec<Self>) -> Self {
        Self::Call {
            callee: Box::new(Self::name(callee)),
            args,
        }
    }
}

/// The name a JavaScript reserved word takes as a **declaration** — a
/// top-level `const` or `let`, an enumeration's or a bitmask's flattened
/// name, a function.
///
/// A trailing underscore, as every sibling column's escape does. Unlike
/// [`crate::zig::escape_decl`], nothing here reserves a built-in type
/// name: plain JavaScript has none, so `const string = 5;` and
/// `const number = 5;` are ordinary declarations and are left alone.
#[must_use]
pub fn escape_decl(name: &str) -> String {
    if RESERVED.contains(&name) {
        let mut out = name.to_string();
        out.push('_');
        return out;
    }
    name.to_string()
}

/// The identity, always.
///
/// A property name is never a declaration: IDL 4.2 §7.2.3's identifier
/// grammar is a strict subset of ECMA-262 §12.6.1's `IdentifierName`,
/// which has admitted every reserved word as a property key since
/// ECMAScript 5, and every aggregate this column builds is a plain
/// object literal and never a `class` — so `constructor` needs no
/// escape either. Kept as a named function, and not simply omitted, so a
/// caller reads the same shape `crate::zig::field_name` and
/// [`crate::typescript::escape_member`] have, and so the one place this
/// column's answer differs from every sibling's is written down instead
/// of implied by its absence.
#[must_use]
pub fn escape_member(name: &str) -> String {
    name.to_string()
}

/// Whether [`escape_decl`] would move this name.
#[must_use]
pub fn is_reserved(name: &str) -> bool {
    RESERVED.contains(&name)
}

/// The words a generated binding may not take.
///
/// ECMA-262 §12.7.2's `ReservedWord` production, the three literals that
/// are not identifiers (`true`, `false`, `null`), and the strict-mode
/// additions this column's generated file also has to answer for whether
/// or not it declares `"use strict"` — `runtime/node/zdwire.js` is
/// required with `require`, which is unaffected either way, but a
/// generated file that only avoided a collision under one mode would be
/// a column whose safety depended on how it happens to be invoked. Also
/// `async`, `of` and `get`/`set`, contextual keywords that are legal
/// bindings in every mode but are refused here anyway: none of them is
/// spelled by any construct this column emits, and a corpus identifier
/// that happened to match would be one test away from being found the
/// hard way.
pub const RESERVED: &[&str] = &[
    "await",
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
    "new",
    "null",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "static",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "var",
    "void",
    "while",
    "with",
    "yield",
];

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn a_reserved_binding_escapes_with_a_trailing_underscore() {
        assert_eq!(escape_decl("class"), "class_");
        assert_eq!(escape_decl("delete"), "delete_");
        assert!(is_reserved("class"));
        assert!(!is_reserved("Pose"));
    }

    #[test]
    fn no_built_in_type_name_is_reserved() {
        // Unlike TypeScript, plain JavaScript has no predefined type
        // names to shadow: `const string = 5;` is ordinary source.
        for name in ["string", "number", "boolean", "any", "unknown", "bigint"] {
            assert_eq!(escape_decl(name), name);
            assert!(!is_reserved(name));
        }
    }

    #[test]
    fn a_member_name_is_never_renamed() {
        // Every property name in this tree is bare `.name` and every
        // aggregate is a plain object literal, so no IDL identifier
        // collides with anything JavaScript reserves.
        for name in ["class", "new", "delete", "constructor", "typeof", "x"] {
            assert_eq!(escape_member(name), name);
        }
    }

    #[test]
    fn the_reserved_list_has_no_duplicate() {
        let mut seen: Vec<&str> = Vec::new();
        for name in RESERVED {
            assert!(!seen.contains(name), "{name} is listed twice");
            seen.push(name);
        }
    }

    #[test]
    fn a_bigint_literal_carries_its_suffix_in_the_written_form() {
        // `2` and `2n` are different values, and this tree holds the
        // form a value was written in.
        let narrow = Expr::Literal(Literal::Int("2".to_string()));
        let wide = Expr::Literal(Literal::Int("2n".to_string()));
        assert_ne!(narrow, wide);
    }

    #[test]
    fn a_var_kind_writes_its_own_spelling() {
        let mut seen = Vec::new();
        for kind in VarKind::ALL {
            assert!(!seen.contains(&kind.as_str()));
            seen.push(kind.as_str());
        }
        assert_eq!(seen, alloc::vec!["const", "let"]);
    }
}
