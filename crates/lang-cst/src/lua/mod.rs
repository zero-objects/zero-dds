// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Lua concrete syntax tree. Spec §1.
//!
//! Lua has no declarations. There is no `struct`, no `type`, no `class`
//! and no annotation the language itself reads; a chunk is a run of
//! statements, and every compound value in it is one thing — a table.
//! So this tree is smaller than any of the other nine, and what it holds
//! is the shape a **data** file takes: a local, an assignment, a
//! function, a table constructor, and a `return`.
//!
//! Three consequences, and each is Lua's rather than a simplification:
//!
//! 1. **There is no type node.** [`Expr`] has no counterpart to
//!    `zerodds_lang_cst::go::Type`, because nothing in a Lua source file
//!    ever states a type. Where the other columns put a width or a bound
//!    into a declaration, this column puts it into a table the generated
//!    file builds — see `zerodds_lang_rules::adapter::lua`, which is
//!    where that decision is written down.
//! 2. **A table key is a string and any string will do.** Lua restricts
//!    the *syntax* `t.name` to identifiers that are not keywords
//!    (Lua 5.4 reference §3.4.9), and not the key itself: `t["end"]` and
//!    `t.end` would name the same key if the second parsed. [`Key`]
//!    therefore has both forms and [`table_key`] picks between them, so
//!    an IDL member never has to be renamed to be reachable. No other
//!    column in this crate can say that.
//! 3. **Comments come in two kinds and the difference is real.** A `--`
//!    comment is read by nothing. A `---` comment is read by
//!    `lua-language-server`, which is where `---@class` and `---@field`
//!    go. Neither is read by the Lua runtime, so both are
//!    [`Stat::Comment`] and [`Stat::Annotation`] rather than one node:
//!    a `---` block attaches to whatever declaration follows it, and a
//!    plain note that attached itself to the next declaration would be
//!    documentation nobody wrote.
//!
//! ## What is here beyond the data forms, and why
//!
//! The tree began as data only — no `if`, no `while`, no `for` — because
//! the materialiser emits descriptors and constructors and nothing that
//! branches. The codec backend (`zerodds_lang_rules::emit::lua`) changed
//! that: a generated decoder is a dispatch on member ids and a generated
//! encoder loops over a sequence, so the tree now carries exactly the
//! control forms the two shared walks ask for — [`Stat::If`],
//! [`Stat::While`], [`Stat::NumericFor`], [`Stat::Break`], a call
//! evaluated as a statement ([`Stat::Perform`]), a unary operator and a
//! deliberate parenthesis. Nothing else was added: there is still no
//! `goto`, no `repeat` and no anonymous function, because no walk asks
//! for one. Every form has a renderer test, so no arm of
//! `zerodds_lang_render::lua` is code that only claims to work.

use crate::common::{Doc, Ident, Literal};
use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// A Lua source file: a chunk that returns one value.
///
/// [`Self::returns`] is not optional. A Lua module is a chunk whose
/// value is what `require` binds (Lua 5.4 reference §6.3, `require`), and
/// a generated file that returned nothing would load, run and hand back
/// `true` — a module with every type in it and no way to reach one. The
/// tree makes that unrepresentable rather than leaving it to a test.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct File {
    /// The header, written with `--` above everything else.
    pub doc: Doc,
    /// The statements, in written order.
    pub body: Vec<Stat>,
    /// What the chunk returns.
    pub returns: Expr,
    /// `@verbatim(placement = END_FILE)` text, after the `return`: OMG
    /// IDL 4.2 §8.3.5.1 puts it at the end of the file, and the `return`
    /// is part of the file.
    pub end_text: Vec<String>,
}

/// One statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stat {
    /// `local name = value`, or `local name` with no value.
    Local {
        /// The `---` block above it.
        doc: Doc,
        /// The name bound.
        name: Ident,
        /// The value, absent for a bare `local name`.
        value: Option<Expr>,
    },
    /// `target = value`.
    Assign {
        /// The `---` block above it.
        doc: Doc,
        /// Where the value goes.
        target: Place,
        /// The value.
        value: Expr,
    },
    /// `function a.b.c(params) … end`.
    Function(Function),
    /// `return value`, or a bare `return`.
    ///
    /// Inside a function body. What the chunk itself returns is
    /// [`File::returns`], so a file cannot be built without one.
    Return(Option<Expr>),
    /// A note written with `--`, read by nothing.
    Comment(Doc),
    /// A block written with `---`, read by `lua-language-server`.
    ///
    /// Separate from [`Stat::Comment`] because the two are not
    /// interchangeable: `lua-language-server` attaches a `---` block to
    /// the declaration that follows it, so a plain note written with
    /// `---` would become documentation for a declaration nobody wrote
    /// it about.
    Annotation(Doc),
    /// `@verbatim` text, copied into the output unchanged — OMG IDL 4.2
    /// §8.3.5.1 asks for the text "verbatim", so raw lines and never a
    /// comment.
    Verbatim(String),
    /// An expression evaluated for its effect.
    ///
    /// Lua's grammar admits only a *functioncall* here (reference
    /// §3.3.6) — `x + 1` on a line of its own does not parse — and every
    /// statement the codec walk performs is one: a runtime call whose
    /// returned handle is deliberately not kept. The tree does not
    /// enforce the restriction; a builder that put a non-call here has
    /// written a chunk `lua` refuses by line number, which is the loud
    /// failure this crate prefers to a silent rewrite.
    Perform(Expr),
    /// `if condition then … elseif … else … end`.
    ///
    /// The `elseif` chain is the renderer's reading of an `otherwise`
    /// that holds exactly one further `If`: the tree nests, because that
    /// is what the walk builds, and the rendering flattens so a
    /// `@mutable` aggregate of a hundred members does not nest a hundred
    /// blocks deep.
    If {
        /// The test.
        condition: Expr,
        /// The statements taken when it holds.
        then: Vec<Stat>,
        /// The statements taken when it does not, absent for a bare `if`.
        otherwise: Option<Vec<Stat>>,
    },
    /// `while condition do … end`.
    While {
        /// The test, evaluated before every pass.
        condition: Expr,
        /// The body.
        body: Vec<Stat>,
    },
    /// `for counter = from, to do … end` — Lua's numeric `for`, whose
    /// upper bound is **inclusive** (reference §3.3.5).
    ///
    /// The codec walk counts from zero exclusive of its bound, so the
    /// backend writes `from = 0, to = bound - 1` and the difference in
    /// convention is decided there, once, rather than at every loop.
    NumericFor {
        /// The counter, local to the loop.
        counter: Ident,
        /// The first value.
        from: Expr,
        /// The last value, inclusive.
        to: Expr,
        /// The body.
        body: Vec<Stat>,
    },
    /// `break`.
    Break,
}

/// `function target(params) … end`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    /// The `---` block above it.
    pub doc: Doc,
    /// The name it is assigned to.
    pub target: Place,
    /// The parameters, in written order.
    pub params: Vec<Ident>,
    /// The body, in written order.
    pub body: Vec<Stat>,
}

/// Somewhere a value can be assigned: a name, or a table entry of one.
///
/// A separate type from [`Expr`] although Lua's grammar calls both a
/// *prefixexp*. Keeping them apart is what stops a rule building
/// `f() = 1`: the tree can only hold the left-hand sides Lua accepts
/// (reference §3.3.3, `varlist`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Place {
    /// `name`.
    Name(Ident),
    /// `base.name`.
    ///
    /// `name` must be an identifier that is not a keyword. `field`
    /// is what enforces that, by answering with [`Place::Index`] where
    /// it does not hold.
    Field {
        /// What it is a field of.
        base: Box<Place>,
        /// The field name, as written after the dot.
        name: Ident,
    },
    /// `base[key]`.
    Index {
        /// What it is an entry of.
        base: Box<Place>,
        /// The key expression.
        key: Box<Expr>,
    },
    /// `(expr)` as the base of a field or an index.
    ///
    /// Lua's *prefixexp* admits a parenthesised expression (reference
    /// §3.4), so `(f())[k]` and `(f()).name` are places to read from.
    /// **Not a place to assign to on its own**: `(x) = 1` is not in
    /// Lua's `varlist`, so a builder may use this only as a base under
    /// [`Place::Field`] or [`Place::Index`], where the grammar takes it.
    Paren(Box<Expr>),
}

impl Place {
    /// A place spelled by one name.
    #[must_use]
    pub fn name(text: &str) -> Self {
        Self::Name(Ident::new(text))
    }

    /// `self.name`, in whichever of the two forms Lua accepts for it.
    ///
    /// The dotted form where `name` is an identifier and not a keyword,
    /// and the bracket form otherwise. The **key is the same string**
    /// either way — Lua's restriction is on the syntax `t.end` and not
    /// on the key `"end"` (reference §3.4.9) — so an IDL member is never
    /// renamed to be reachable.
    #[must_use]
    pub fn field(self, name: &str) -> Self {
        if is_identifier(name) {
            return Self::Field {
                base: Box::new(self),
                name: Ident::new(name),
            };
        }
        Self::Index {
            base: Box::new(self),
            key: Box::new(Expr::Literal(Literal::string(name))),
        }
    }
}

/// An expression.
///
/// Only the forms a data file takes. There is no arithmetic beyond
/// [`Expr::Binary`], which exists because a bitmask constant is `1 << 4`
/// and writing `16` instead would put a number in the file that the IDL
/// did not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    /// `nil`.
    ///
    /// Written on purpose where it is written at all. A table entry
    /// whose value is `nil` is an entry that does not exist, so a rule
    /// that means "absent" says so by leaving the entry out — this is
    /// for the places where the word itself is the value.
    Nil,
    /// A literal.
    Literal(Literal),
    /// A name, a field or an index.
    Place(Place),
    /// `{ … }`.
    Table(Table),
    /// `func(args)`.
    ///
    /// The callee is a [`Place`] and not an arbitrary expression: every
    /// call a generated data file makes is to a named function —
    /// `assert`, or a constructor this file declared.
    Call {
        /// What is called.
        func: Box<Place>,
        /// The arguments, in written order.
        args: Vec<Expr>,
    },
    /// `lhs op rhs`.
    Binary {
        /// The operator, as written.
        op: &'static str,
        /// The left operand.
        lhs: Box<Expr>,
        /// The right operand.
        rhs: Box<Expr>,
    },
    /// `op value` — `not`, `~`, `#` or `-`.
    ///
    /// The renderer puts a space after `not` because `notx` is an
    /// identifier, and none after the symbol operators.
    Unary {
        /// The operator, as written.
        op: &'static str,
        /// The operand.
        value: Box<Expr>,
    },
    /// `(value)` — a parenthesis written on purpose.
    ///
    /// The tree carries no precedence, so a builder that needs a bracket
    /// says so and the renderer honours it. The codec walks bracket
    /// deliberately ([`CodecTarget::paren`]); nothing brackets by guess.
    ///
    /// [`CodecTarget::paren`]: ../../zerodds_lang_rules/emit/trait.CodecTarget.html#tymethod.paren
    Paren(Box<Expr>),
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

    /// A name, a field or an index, as an expression.
    #[must_use]
    pub const fn place(place: Place) -> Self {
        Self::Place(place)
    }
}

/// A table constructor: `{ … }`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Table {
    /// The entries, in written order.
    ///
    /// Order is the whole point of this node. A Lua table has none —
    /// `next`, and therefore `pairs`, "does not specify any order for
    /// the traversal" (Lua 5.4 reference §6.1) — so a member list that
    /// has to keep declaration order is written as the **array part** of
    /// a table, where the index is the order.
    pub entries: Vec<Entry>,
}

impl Table {
    /// A table of these entries.
    #[must_use]
    pub const fn of(entries: Vec<Entry>) -> Self {
        Self { entries }
    }

    /// Whether it has no entries, which renders as `{}`.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// One entry of a table constructor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The `---` block above it, for an entry written on its own line.
    pub doc: Doc,
    /// How it is keyed.
    pub key: Key,
    /// The value.
    pub value: Expr,
}

impl Entry {
    /// `key = value`, in whichever of the two forms Lua accepts.
    #[must_use]
    pub fn named(key: &str, value: Expr) -> Self {
        Self {
            doc: Doc::none(),
            key: table_key(key),
            value,
        }
    }

    /// One element of the array part.
    #[must_use]
    pub fn positional(value: Expr) -> Self {
        Self {
            doc: Doc::none(),
            key: Key::Positional,
            value,
        }
    }
}

/// How a table entry is keyed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Key {
    /// No key: the entry joins the array part, at the next integer.
    ///
    /// **The array part starts at 1**, which is Lua's own convention and
    /// what the length operator and `ipairs` assume (Lua 5.4 reference
    /// §3.4.7). An IDL sequence index is 0-based, so the two differ by
    /// one everywhere they meet.
    Positional,
    /// `name = value`.
    ///
    /// `name` must be an identifier that is not a keyword; [`table_key`]
    /// is what guarantees it.
    Name(Ident),
    /// `[key] = value`.
    Expr(Expr),
}

/// The key form a name takes in a table constructor.
///
/// [`Key::Name`] where the name is a Lua identifier that is not a
/// keyword, and `["…"]` otherwise. The two produce the **same key**: a
/// Lua table is keyed by the string, and `name = v` is defined as
/// `["name"] = v` (Lua 5.4 reference §3.4.9). So an IDL identifier is
/// never renamed to be written — which is the one thing this column has
/// that the other nine do not, and the reason there is no `escape_name`
/// in this module.
#[must_use]
pub fn table_key(name: &str) -> Key {
    if is_identifier(name) {
        return Key::Name(Ident::new(name));
    }
    Key::Expr(Expr::Literal(Literal::string(name)))
}

/// Whether `name` may be written after a dot or before an `=` in a table
/// constructor.
///
/// Lua's own rule: a letter or underscore, then letters, digits and
/// underscores, and not one of the keywords (Lua 5.4 reference §3.1).
/// Only ASCII letters count — the reference makes the letter set locale
/// dependent in principle and every current implementation ASCII in
/// practice, and a generated file must not depend on which.
#[must_use]
pub fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    if !chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_') {
        return false;
    }
    !KEYWORDS.contains(&name)
}

/// Every Lua keyword.
///
/// The twenty-two of Lua 5.4 reference §3.1, `goto` included — it became
/// a keyword in Lua 5.2, so a file that used it as a field name would
/// parse under 5.1 and not under 5.4.
///
/// This list decides syntax and nothing else. A member named `end` keeps
/// the name `end`; it is written `t["end"]` instead of `t.end`.
pub const KEYWORDS: &[&str] = &[
    "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "goto", "if", "in",
    "local", "nil", "not", "or", "repeat", "return", "then", "true", "until", "while",
];

/// The text a Lua **float** literal has to have.
///
/// Lua 5.4 has integers and floats as two subtypes of `number`, and a
/// numeral decides which it is by how it is written: one "with a radix
/// point or an exponent denotes a float; otherwise, if its value fits in
/// an integer or it is a hexadecimal numeral, it denotes an integer"
/// (reference §3.1). So `0` is an integer and `0.0` is a float, and a
/// member holding an IDL `double` written as `0` would answer `"integer"`
/// to `math.type`.
///
/// Measured with the interpreter on the path:
/// `lua -e 'print(math.type(0), math.type(0.0))'` prints
/// `integer` and `float`.
#[must_use]
pub fn float_text(text: &str) -> String {
    let looks_float = text
        .chars()
        .any(|ch| ch == '.' || ch == 'e' || ch == 'E' || ch == 'n' || ch == 'i');
    if looks_float {
        return text.to_string();
    }
    let mut out = text.to_string();
    out.push_str(".0");
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;
    use alloc::vec;

    #[test]
    fn a_keyword_is_not_written_after_a_dot() {
        assert!(!is_identifier("end"));
        assert!(!is_identifier("local"));
        assert!(!is_identifier("nil"));
        assert!(is_identifier("payload"));
        assert!(is_identifier("_d"));
    }

    #[test]
    fn a_name_that_is_not_an_identifier_is_not_written_after_a_dot() {
        assert!(!is_identifier(""));
        assert!(!is_identifier("2fast"));
        assert!(!is_identifier("has space"));
        assert!(!is_identifier("a-b"));
    }

    #[test]
    fn a_keyword_keeps_its_name_and_changes_only_its_form() {
        // The whole reason this column has no `escape_name`. `t.end` does
        // not parse and `t["end"]` does, and both would name the key
        // `"end"` — so nothing is renamed.
        match table_key("end") {
            Key::Expr(Expr::Literal(Literal::Str(text))) => assert_eq!(text, "end"),
            other => panic!("{other:?} is not the bracket form"),
        }
        match table_key("x") {
            Key::Name(name) => assert_eq!(name.as_str(), "x"),
            other => panic!("{other:?} is not the dotted form"),
        }
    }

    #[test]
    fn a_place_takes_the_bracket_form_for_a_keyword_and_the_dot_form_otherwise() {
        let dotted = Place::name("M").field("Pose");
        assert!(matches!(dotted, Place::Field { .. }));
        let bracketed = Place::name("M").field("end");
        assert!(matches!(bracketed, Place::Index { .. }));
    }

    #[test]
    fn the_keyword_list_has_no_duplicates_and_holds_goto() {
        let mut seen: Vec<&str> = Vec::new();
        for name in KEYWORDS {
            assert!(!seen.contains(name), "{name} is listed twice");
            seen.push(name);
        }
        // A keyword since Lua 5.2. A file using it as a field name would
        // parse under 5.1 and not under 5.4.
        assert!(KEYWORDS.contains(&"goto"));
        assert_eq!(KEYWORDS.len(), 22);
    }

    #[test]
    fn a_float_literal_is_written_so_that_math_type_answers_float() {
        // Lua 5.4 reference §3.1: a numeral with a radix point or an
        // exponent is a float, and one without is an integer.
        assert_eq!(float_text("0"), "0.0");
        assert_eq!(float_text("-3"), "-3.0");
        assert_eq!(float_text("1.5"), "1.5");
        assert_eq!(float_text("1e10"), "1e10");
        assert_eq!(float_text("1E10"), "1E10");
    }

    #[test]
    fn a_table_keeps_its_entries_in_the_order_they_were_added() {
        // A Lua table has no order — `next` "does not specify any order
        // for the traversal" (reference §6.1) — so the array part is
        // where a member list keeps declaration order.
        let table = Table::of(vec![
            Entry::positional(Expr::string("first")),
            Entry::positional(Expr::string("second")),
        ]);
        assert_eq!(table.entries.len(), 2);
        assert_eq!(table.entries[0].value, Expr::string("first"));
        assert_eq!(table.entries[1].value, Expr::string("second"));
    }

    #[test]
    fn an_empty_table_is_representable() {
        assert!(Table::default().is_empty());
    }
}
