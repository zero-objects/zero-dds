// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Python concrete syntax tree. Spec §1.
//!
//! Python has no braces, no declarations and no type syntax of its own —
//! an annotation is an ordinary expression. The tree therefore differs
//! from the other four in three ways, and each is Python's rather than a
//! simplification:
//!
//! 1. **A block is a list of statements and nothing else.** There is no
//!    delimiter to get wrong, so [`Class`] carries a `body` and the
//!    renderer owns the indentation. A body that renders to nothing is
//!    not legal Python, which is why [`Statement::Pass`] exists as a node
//!    a rule emits rather than something the renderer inserts.
//! 2. **[`Type`] is separate from [`Expr`] although Python spells them
//!    the same.** Keeping them apart is what stops a rule building
//!    `Optional[a, b]` or `list[1]`: the tree can only hold the forms an
//!    annotation actually takes. Where an annotation genuinely needs a
//!    value — the metadata of a [`Type::Annotated`] — an [`Expr`] appears
//!    inside the type, which is exactly what `typing.Annotated` is for.
//! 3. **There is no visibility and no modifier.** Python has neither, so
//!    [`crate::common::Visibility`] is not used here; a leading
//!    underscore is a convention and is part of the identifier.
//!
//! ## What is here, and what the second pass added
//!
//! The tree started as the *types* an IDL file becomes and nothing else.
//! The wire codecs are the second pass, and they are what
//! [`Function`], [`Statement::If`], [`Statement::For`],
//! [`Statement::While`], [`Statement::Break`], [`Statement::Return`],
//! [`Statement::Set`], [`Expr::Attribute`], [`Expr::Index`],
//! [`Expr::Unary`], [`Expr::Paren`] and [`Expr::List`] arrived for:
//! `crates/lang-rules/src/emit/python.rs` builds every one of them out
//! of the shared walk, and each has a renderer test in
//! `zerodds_lang_render::python`.
//!
//! What is here is still bounded by that rule in the other direction —
//! every form has a renderer test, so no arm of
//! `zerodds_lang_render::python` is code that only claims to work. A
//! field or a variant with no test and no producer would be exactly
//! that, which is why [`Class`] has no `metaclass=` and [`Function`] has
//! no `*args`, no `**kwargs` and no `async`.
//!
//! ## A binding and a store are two statements, not one
//!
//! Python spells both `x = y`. They are apart here — [`Statement::Assign`]
//! binds a *name* and [`Statement::Set`] writes into a *place* — because
//! only one of them can be either. `RED = 0` inside an `enum.IntEnum` is
//! a member and has to be a name; `v.field = x` and `v[i] = x` are the
//! codec's, and neither is a name a scope binds. A single node taking an
//! arbitrary target would let a materialiser declare an enumeration
//! member called `v[0]`, and nothing between it and the file would
//! notice.

use crate::common::{Doc, Ident, Literal, Path};
use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// A Python source file.
///
/// One module per IDL file. Python's own unit of scope is the module, so
/// unlike the Java tree there is no outer type everything nests inside:
/// a top-level IDL declaration is a top-level Python statement.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Module {
    /// The module docstring.
    pub doc: Doc,
    /// `from __future__ import ...`, which must precede every other
    /// statement, so it is a field rather than one more import.
    pub future: Vec<Ident>,
    /// `import` lines, in written order.
    pub imports: Vec<Import>,
    /// The body, in written order.
    pub body: Vec<Statement>,
}

/// One `import` line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Import {
    /// `import a.b [as c]`
    Module {
        /// What is imported.
        path: Path,
        /// The name it is bound to, when it is not the path's own.
        alias: Option<Ident>,
    },
    /// `from a.b import c [as d], e`
    From {
        /// The module imported from.
        path: Path,
        /// What is taken out of it, in written order.
        names: Vec<ImportName>,
    },
}

/// One name in a `from ... import ...`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportName {
    /// The name in the source module.
    pub name: Ident,
    /// The name it is bound to, when it differs.
    pub alias: Option<Ident>,
}

/// One statement of a module or a class body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    /// A class definition.
    Class(Class),
    /// `name: Type` or `name: Type = value` — an annotated binding.
    ///
    /// One node for a dataclass field, a `Final` constant and a bitset's
    /// bitfield: Python spells all three the same way, and a tree that
    /// distinguished them would be recording the rule's intent rather
    /// than the syntax.
    Annotated(Annotated),
    /// `name = value` — a binding with no annotation.
    ///
    /// What a type alias and an enumeration member take: `RED = 0` inside
    /// an `enum.IntEnum` is a member and `RED: int = 0` is not, so the
    /// two forms are not interchangeable.
    Assign(Assign),
    /// A function definition.
    Function(Function),
    /// `place = value` — a store into somewhere that already exists.
    ///
    /// `v.field = x`, `v[i] = x`, `zd_wire = 3`. Distinct from
    /// [`Statement::Assign`] for the reason the module documentation
    /// gives: a binding introduces a name and a store does not, and only
    /// the first of the two may appear where an enumeration member is
    /// read off the class body.
    Set {
        /// Where the value goes.
        place: Expr,
        /// The value.
        value: Expr,
    },
    /// An expression evaluated for its effect.
    ///
    /// What a call to the runtime is: `w.write_u32(v.x)`. Python has no
    /// `(void)` cast and no discarded-result diagnostic, so a value the
    /// generated code reads and does not keep is written as the call and
    /// nothing else.
    Expr(Expr),
    /// `if condition: ... [else: ...]`
    ///
    /// No `elif`. A chain is an `else` whose body is one more
    /// [`Statement::If`], which is what `elif` abbreviates and what the
    /// codec walk builds; a separate arm would be a second spelling of
    /// one tree.
    If {
        /// The condition.
        condition: Expr,
        /// The body taken when it holds; never empty.
        then: Vec<Statement>,
        /// The body taken when it does not.
        otherwise: Option<Vec<Statement>>,
    },
    /// `for counter in range(bound): ...`
    ///
    /// A counted loop and not the general `for x in y`. The codec walk's
    /// `CodecTarget::count_up` asks for a count from zero and nothing
    /// else; a tree that could hold any iterable would be recording a
    /// loop this column never writes.
    For {
        /// The induction variable.
        counter: Ident,
        /// The exclusive bound.
        bound: Expr,
        /// The body; never empty.
        body: Vec<Statement>,
    },
    /// `while condition: ...`
    While {
        /// The condition.
        condition: Expr,
        /// The body; never empty.
        body: Vec<Statement>,
    },
    /// `break`.
    Break,
    /// `return`, with or without a value.
    Return(Option<Expr>),
    /// A `#` comment on lines of its own.
    Comment(Doc),
    /// `pass`.
    ///
    /// A body has to hold something. A rule that produced an empty class
    /// emits this rather than leaving the renderer to notice, because a
    /// renderer that inserted it would be deciding what the tree says.
    Pass,
    /// `@verbatim` text, copied into the output unchanged — OMG IDL 4.2
    /// §8.3.5.1 asks for the text "verbatim", so raw lines and never a
    /// comment. At the indentation of the suite it stands in.
    Verbatim(String),
}

/// `def name(params) -> returns: ...`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    /// The docstring, written as the first statement of the body.
    pub doc: Doc,
    /// Decorators, in written order, outermost first.
    pub decorators: Vec<Decorator>,
    /// The name.
    pub name: Ident,
    /// The parameters, in written order.
    pub params: Vec<Param>,
    /// The return annotation. `None` leaves it unwritten, which is not
    /// the same as `-> None`: the first says nothing and the second says
    /// the function answers with nothing.
    pub returns: Option<Type>,
    /// The body, in written order; never empty.
    pub body: Vec<Statement>,
}

/// One parameter of a [`Function`].
///
/// No `*args`, no `**kwargs` and no positional-only marker: a generated
/// codec takes a fixed list and a field nothing sets would be a renderer
/// arm nothing reviews.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    /// The name.
    pub name: Ident,
    /// The annotation, absent where the parameter carries none.
    pub type_: Option<Type>,
    /// The default value, absent where it has none.
    pub default: Option<Expr>,
}

/// `name: Type` or `name: Type = value`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Annotated {
    /// Comment lines written above it.
    pub doc: Doc,
    /// The name bound.
    pub name: Ident,
    /// The annotation.
    pub type_: Type,
    /// The value, absent for a declaration with no initialiser.
    pub value: Option<Expr>,
}

/// `name = value`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assign {
    /// Comment lines written above it.
    pub doc: Doc,
    /// The name bound.
    pub name: Ident,
    /// The value.
    pub value: Expr,
}

/// `class Name(bases): ...`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Class {
    /// The docstring, written as the first statement of the body.
    pub doc: Doc,
    /// Decorators, in written order, outermost first.
    pub decorators: Vec<Decorator>,
    /// The name.
    pub name: Ident,
    /// Base classes, in written order.
    ///
    /// No class keywords beside them: `metaclass=` and `boundary=` have
    /// no part in a type declaration, and a field nothing sets would be
    /// a renderer arm nothing reviews.
    pub bases: Vec<Type>,
    /// The body, in written order.
    pub body: Vec<Statement>,
}

/// One decorator: `@path(args)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decorator {
    /// The name applied.
    pub path: Path,
    /// Its arguments; empty means the decorator is written bare, with no
    /// parentheses at all. `@dataclasses.dataclass` and
    /// `@dataclasses.dataclass()` are the same thing, and the bare form
    /// is the conventional one.
    pub args: Vec<Arg>,
}

/// One argument of a call or a decorator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Arg {
    /// A positional argument.
    Positional(Expr),
    /// `name=value`
    Keyword(Keyword),
}

/// `name=value`, in a call, a decorator or a class header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keyword {
    /// The parameter's name.
    pub name: Ident,
    /// The value.
    pub value: Expr,
}

/// A type annotation.
///
/// Python writes an annotation as an expression, so every form here is
/// also an [`Expr`] as far as the grammar is concerned. They are separate
/// types anyway: a rule that could put an arbitrary expression where an
/// annotation belongs could put `list[1]` there, and nothing between the
/// rule and the file would notice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    /// A name or a dotted name: `int`, `a.b.Point`.
    Name(Path),
    /// `base[arg, ...]` — `list[int]`, `tuple[int, int]`.
    Subscript {
        /// What is subscripted.
        base: Path,
        /// The arguments, in written order.
        args: Vec<Type>,
    },
    /// `typing.Optional[T]`.
    ///
    /// Its own form rather than a [`Type::Subscript`]: `Optional` takes
    /// exactly one argument, and the tree is where that is true.
    Optional(Box<Type>),
    /// `typing.Annotated[T, m, ...]` — the type, and what it means.
    ///
    /// The one place an [`Expr`] appears inside a [`Type`], because that
    /// is what `typing.Annotated` is: a type paired with values a static
    /// checker passes through and a reader at run time can read back with
    /// `typing.get_type_hints(..., include_extras=True)`.
    Annotated {
        /// The type a checker sees.
        base: Box<Type>,
        /// The metadata, in written order; never empty.
        markers: Vec<Expr>,
    },
}

impl Type {
    /// A type spelled by one name.
    #[must_use]
    pub fn name(text: &str) -> Self {
        Self::Name(Path::single(text))
    }

    /// `base[args]`.
    #[must_use]
    pub fn subscript(base: Path, args: Vec<Self>) -> Self {
        Self::Subscript { base, args }
    }
}

/// An expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    /// A literal.
    Literal(Literal),
    /// A name or a dotted name.
    Path(Path),
    /// `func(args)`.
    Call {
        /// What is called.
        func: Path,
        /// The arguments, in written order.
        args: Vec<Arg>,
    },
    /// `(a, b)`.
    Tuple(Vec<Expr>),
    /// `[a, b]`.
    ///
    /// A list and not a tuple, which is a different value: a tuple
    /// cannot be written into, and the reader's collection slots are
    /// exactly what has to be.
    List(Vec<Expr>),
    /// `{k: v}`.
    Dict(Vec<(Expr, Expr)>),
    /// `value.name` — an attribute of something that is not a bare name.
    ///
    /// [`Expr::Path`] already spells `a.b.C`, and that is a dotted
    /// *name*: every segment is an identifier. This is the other thing —
    /// an attribute of a computed value, `v[i].key`, `zd_h.member_id` —
    /// which a path cannot hold because its object is not a name.
    Attribute {
        /// What the attribute is taken of.
        value: Box<Expr>,
        /// The attribute.
        name: Ident,
    },
    /// `value[index]`.
    Index {
        /// What is subscripted.
        value: Box<Expr>,
        /// The subscript.
        index: Box<Expr>,
    },
    /// `start:end` — a slice, which is a subscript and nothing else.
    ///
    /// Only ever the `index` of an [`Expr::Index`]; Python has no other
    /// position a slice is an expression in. Both ends are optional and
    /// an absent one is written as nothing, which is what `v[:n]` is.
    /// No step: §7.3.1.2.1.12.3's TRIM keeps a prefix, and a stride
    /// would be a form with no producer.
    Slice {
        /// Where the slice starts, absent for the beginning.
        start: Option<Box<Expr>>,
        /// Where it ends, absent for the end.
        end: Option<Box<Expr>>,
    },
    /// `op value`, written as given.
    ///
    /// `not x` and `~x`. The operator carries its own trailing space
    /// where Python needs one, because `notx` is an identifier and
    /// `~x` takes none — a renderer that inserted a space would be
    /// deciding which of the two this is.
    Unary {
        /// The operator, as written, including any trailing space.
        op: &'static str,
        /// The operand.
        value: Box<Expr>,
    },
    /// Parentheses written on purpose.
    ///
    /// [`Expr::Binary`] brackets itself, so this is for the forms that
    /// do not: a [`Expr::Unary`] whose operand binds more loosely than
    /// the operator, and anywhere the walk asks for a bracket outright.
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
    /// A type written where a value belongs.
    ///
    /// Which is exactly what a Python type alias is: `X = list[int]` is
    /// an assignment whose right-hand side is an annotation. Without
    /// this the two sides would need two spellings of the same syntax,
    /// and they could drift.
    Annotation(Box<Type>),
}

impl Expr {
    /// An expression that is one name.
    #[must_use]
    pub fn name(text: &str) -> Self {
        Self::Path(Path::single(text))
    }

    /// An integer literal.
    #[must_use]
    pub fn int(value: i128) -> Self {
        Self::Literal(Literal::int(value))
    }
}

/// The name an IDL identifier takes in Python.
///
/// A trailing underscore where the identifier would otherwise bind over
/// something the generated file itself relies on. PEP 8 gives that rule
/// for the keyword case — `class_`, `lambda_` — and the same escape
/// covers the other two cases, because in Python they are the same
/// problem:
///
/// - a **keyword**, which does not parse as a name at all;
/// - a **soft keyword** (`match`, `case`, `type`), which parses but reads
///   as a statement to anyone scanning the file;
/// - a **name the generated module binds** — `dataclasses`, `typing`,
///   `int`, `list` — which would be shadowed. This is not hypothetical
///   and it is not caught by the parser: an `@optional` member called
///   `int` becomes a class attribute set to `None`, and
///   `typing.get_type_hints` resolves annotations against the class
///   namespace first, so every `int` in that class would then resolve to
///   `None`;
/// - **`mro`**, which `enum.EnumType` refuses as a member name. It is
///   the only such name — `name`, `value`, `property` and `auto` are all
///   accepted, measured against CPython 3.14 — and an enumeration
///   carrying it raises `ValueError` when the module is imported.
///
/// The IDL name is not lost when this fires: the materialiser writes it
/// into the field's `dataclasses.field(metadata=...)` and into a comment
/// above every other kind of declaration.
///
/// A leading underscore is left alone. It is a legal Python identifier
/// and renaming it would make the generated name differ from the IDL for
/// no reason.
#[must_use]
pub fn escape_name(name: &str) -> String {
    if RESERVED.contains(&name) {
        let mut out = name.to_string();
        out.push('_');
        return out;
    }
    name.to_string()
}

/// Every name an escape has to move out of the way of.
///
/// The hard keywords and the soft ones are the language's (Python 3.13
/// reference §2.3.1); the rest are the names a generated module binds or
/// writes, which is why they are here and not only in a materialiser: a
/// second backend spelling this set differently would produce two files
/// that disagree about which names are safe.
pub const RESERVED: &[&str] = &[
    // Keywords.
    "False",
    "None",
    "True",
    "and",
    "as",
    "assert",
    "async",
    "await",
    "break",
    "class",
    "continue",
    "def",
    "del",
    "elif",
    "else",
    "except",
    "finally",
    "for",
    "from",
    "global",
    "if",
    "import",
    "in",
    "is",
    "lambda",
    "nonlocal",
    "not",
    "or",
    "pass",
    "raise",
    "return",
    "try",
    "while",
    "with",
    "yield",
    // Soft keywords: legal as names, and escaped anyway.
    "case",
    "match",
    "type",
    // The modules a generated file imports, and the name
    // `from __future__ import annotations` binds.
    "annotations",
    "dataclasses",
    "enum",
    "typing",
    // Refused by `enum.EnumType` as a member name, so an enumerator of
    // this name would make the whole module fail to import.
    "mro",
    // The builtins a generated annotation is written in terms of.
    "bool",
    "bytes",
    "float",
    "int",
    "list",
    "object",
    "str",
    "tuple",
];

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn a_keyword_escapes_with_a_trailing_underscore() {
        assert_eq!(escape_name("class"), "class_");
        assert_eq!(escape_name("lambda"), "lambda_");
        assert_eq!(escape_name("payload"), "payload");
    }

    #[test]
    fn a_soft_keyword_escapes_although_it_would_parse() {
        // Legal as an identifier, escaped anyway: a member called
        // `match` reads as a statement to anyone scanning the file.
        for name in ["match", "case", "type"] {
            assert_eq!(escape_name(name), alloc::format!("{name}_"));
        }
    }

    #[test]
    fn a_name_the_generated_module_binds_escapes_too() {
        // The case the parser does not catch. An `@optional` member
        // called `int` becomes a class attribute set to `None`, and
        // `typing.get_type_hints` resolves the class namespace before
        // the module's — so every `int` in that class would resolve to
        // `None` rather than to the builtin.
        for name in ["int", "list", "tuple", "str", "typing", "dataclasses"] {
            assert_eq!(escape_name(name), alloc::format!("{name}_"));
        }
    }

    #[test]
    fn the_one_name_an_enumeration_may_not_use_escapes() {
        // `enum.EnumType` refuses it, so an enumerator called `mro`
        // would make the whole generated module fail to import. It is
        // the only one: `name`, `value`, `property` and `auto` are all
        // accepted as members, measured against CPython 3.14.
        assert_eq!(escape_name("mro"), "mro_");
        assert_eq!(escape_name("name"), "name");
        assert_eq!(escape_name("value"), "value");
    }

    #[test]
    fn a_leading_underscore_is_left_alone() {
        // Legal Python, and renaming it would make the generated name
        // differ from the IDL for no reason.
        assert_eq!(escape_name("_reserved"), "_reserved");
        assert_eq!(escape_name("_d"), "_d");
    }

    #[test]
    fn the_reserved_list_has_no_duplicates() {
        // A duplicate would be harmless and would also mean the list was
        // assembled by hand twice.
        let mut seen: Vec<&str> = Vec::new();
        for name in RESERVED {
            assert!(!seen.contains(name), "{name} is listed twice");
            seen.push(name);
        }
    }

    #[test]
    fn an_annotated_type_carries_its_metadata_beside_the_type_a_checker_sees() {
        let type_ = Type::Annotated {
            base: Box::new(Type::name("int")),
            markers: alloc::vec![Expr::Call {
                func: Path::single("IdlInt"),
                args: alloc::vec![Arg::Keyword(Keyword {
                    name: Ident::new("bits"),
                    value: Expr::int(32),
                })],
            }],
        };
        let Type::Annotated { base, markers } = &type_ else {
            panic!("not annotated");
        };
        assert_eq!(**base, Type::name("int"));
        assert_eq!(markers.len(), 1);
    }
}
