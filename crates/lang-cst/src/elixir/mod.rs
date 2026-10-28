// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Elixir concrete syntax tree. Spec §1.
//!
//! Elixir has no declarations in the sense the C family has. A module is
//! a block of expressions that run at compile time, and everything a
//! generated file states is either a **module attribute** — `@type`,
//! `@enforce_keys`, `@callback`, or one this column invents — or a
//! function definition. So the tree holds exactly those two things and
//! the `defstruct` macro call, and nothing else.
//!
//! Three things this tree carries that no sibling's does, and each is
//! Elixir's:
//!
//! 1. **[`Item::Attribute`] is a first-class declaration form.** An
//!    Elixir module attribute is where a generated file writes what the
//!    language cannot enforce, and it is readable at run time only
//!    through a function that returns it — a bare attribute exists at
//!    compile time and is gone afterwards. [`Item::Function`] is
//!    therefore not decoration: it is what makes an attribute
//!    observable, and `elixirc` warns "module attribute @x was set but
//!    never used" for one that has no reader (measured, Elixir 1.20.2).
//! 2. **[`Type`] is separate from [`Expr`] although Elixir spells them
//!    with the same grammar.** A typespec is an ordinary quoted
//!    expression as far as the parser is concerned; keeping the two
//!    apart is what stops a rule writing `list(1)` or `%__MODULE__{}`
//!    where a value belongs.
//! 3. **A module name is a [`crate::common::Path`] and stays one.**
//!    Elixir module names nest with `.` and that nesting is real — the
//!    atom `Elixir.Common.Util.Location` is one name with three
//!    segments — so unlike the C and Go trees nothing here flattens a
//!    scope onto an identifier.
//!
//! ## What is not here
//!
//! No `case`, no `cond`, no pipe, no anonymous function, no `import`,
//! `alias` or `use`. This tree carries the *types* an IDL file becomes,
//! the attributes that say what Elixir cannot, and the zero-arity
//! functions that make those attributes readable. Every form has a
//! renderer test, so no arm of `zerodds_lang_render::elixir` is code
//! that only claims to work.

use crate::common::{Doc, Ident, Literal, Path};
use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// An Elixir source file: one `.ex`, holding one or more modules.
///
/// Elixir puts no constraint on how many modules a file declares and no
/// constraint on their names matching the path, so one IDL file becomes
/// one `.ex` file however many modules it declares. That is why there is
/// no package or namespace field here: the module's own name carries its
/// whole scope.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct File {
    /// The leading `#` comment block.
    pub doc: Doc,
    /// The modules and the text between them, in written order.
    pub modules: Vec<TopLevel>,
}

/// One entry at the top of a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TopLevel {
    /// `defmodule Name do … end`.
    Module(Module),
    /// `@verbatim` text, copied into the output unchanged — OMG IDL 4.2
    /// §8.3.5.1 asks for the text "verbatim", so raw lines and never a
    /// comment. A file holds nothing but modules otherwise, so this is
    /// where text before, after or between them lives.
    Verbatim(String),
}

/// `defmodule Name.Space do … end`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module {
    /// The `@moduledoc`, written as the first thing in the body.
    pub doc: Doc,
    /// The name, outermost segment first.
    ///
    /// A [`Path`] and not an [`Ident`]: `Common.Util.Location` is one
    /// module whose name has three segments, and joining them into a
    /// single identifier would lose the only structure Elixir gives a
    /// generated file for free.
    pub name: Path,
    /// The body, in written order.
    pub body: Vec<Item>,
}

/// One thing a module body holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    /// `@name value` — a module attribute.
    Attribute(Attribute),
    /// `defstruct […]`.
    ///
    /// Its own item and not an attribute: `defstruct` is a macro that
    /// generates `__struct__/0` and `__struct__/1`, and the empty case
    /// `defstruct []` is legal and means a struct with no fields.
    Defstruct(Vec<Field>),
    /// `@type name :: type`.
    TypeAlias(TypeAlias),
    /// `@spec name(types) :: type`.
    Spec(Signature),
    /// `@callback name(types) :: type`.
    ///
    /// The same shape as a [`Item::Spec`] and a different meaning: a
    /// `@spec` describes a function this module defines, a `@callback`
    /// describes one an implementing module must define. A module whose
    /// body is `@callback`s *is* a behaviour.
    Callback(Signature),
    /// `def name(patterns), do: body`.
    Function(Function),
    /// `@verbatim` text inside the module body, copied unchanged.
    Verbatim(String),
}

/// `@name value`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribute {
    /// Comment lines written above it.
    pub doc: Doc,
    /// The attribute's name, without the `@`.
    pub name: Ident,
    /// Its value.
    pub value: Expr,
}

/// One field of a `defstruct`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// The key, which is an atom.
    pub name: Ident,
    /// The default.
    ///
    /// Always written, never a bare `:key`. Elixir's list syntax
    /// requires the keyword pairs of a list to come last, so a
    /// `defstruct` mixing bare keys with defaults could only be written
    /// by moving the bare ones to the front — and the order of the
    /// fields in the declaration is the IDL's, which is the order the
    /// wire uses. A field whose type has no value that could stand as a
    /// default takes `nil` here and is named in `@enforce_keys`, which
    /// makes `%Struct{}` a compile error rather than a sample with a
    /// hole in it.
    pub default: Expr,
}

/// `@type name :: type`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeAlias {
    /// Comment lines written above it.
    pub doc: Doc,
    /// The name declared, without parameters.
    pub name: Ident,
    /// What it names.
    pub type_: Type,
}

/// The parameter and result types of a `@spec` or a `@callback`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    /// Comment lines written above it.
    pub doc: Doc,
    /// The function's name.
    pub name: Ident,
    /// Its parameters, in order.
    pub params: Vec<Param>,
    /// Its result.
    pub result: Type,
}

/// One parameter of a `@spec` or a `@callback`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    /// The name, written as `name :: type` when present.
    ///
    /// Optional because a `@spec` conventionally states types alone
    /// while a `@callback` names them, and both are legal in both.
    pub name: Option<Ident>,
    /// The type.
    pub type_: Type,
}

/// `def name(patterns), do: body`.
///
/// One clause. A function of several clauses is several [`Function`]
/// items with one name, which is how Elixir itself writes it — and they
/// must be adjacent, because Elixir warns "clauses with the same name
/// should be grouped together" when they are not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    /// Comment lines written above it.
    pub doc: Doc,
    /// The name.
    pub name: Ident,
    /// The parameter patterns, in order. Empty writes no parentheses at
    /// all, which is what a zero-arity function conventionally takes.
    pub params: Vec<Expr>,
    /// The body, which is one expression.
    pub body: Expr,
}

/// A typespec.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    /// One of Elixir's own basic types.
    Basic(Basic),
    /// A literal atom, as a type: `:idle`.
    ///
    /// A type inhabited by exactly one value, which is what makes a
    /// union of them a useful enumeration type.
    Atom(String),
    /// A type declared by this module: `t()`.
    Local(Ident),
    /// A type declared by another module: `Common.Util.Location.t()`.
    Remote {
        /// The module.
        module: Path,
        /// The type's name in it.
        name: Ident,
    },
    /// `list(T)`.
    List(Box<Type>),
    /// `{A, B}` — a tuple whose arity is part of the type.
    Tuple(Vec<Type>),
    /// `%__MODULE__{key: T, …}`.
    ///
    /// Always `__MODULE__` and never a named module: a struct type
    /// mentioning a key the module's own `defstruct` does not declare is
    /// a compile error — `undefined field :z on struct Bad` — which is
    /// the one cross-check Elixir performs between the two halves of a
    /// generated declaration.
    Struct(Vec<(Ident, Type)>),
    /// `A | B`.
    Union(Vec<Type>),
}

impl Type {
    /// `list(T)`.
    #[must_use]
    pub fn list(element: Self) -> Self {
        Self::List(Box::new(element))
    }

    /// `T | nil`, which is what an absent value is spelled as.
    #[must_use]
    pub fn or_nil(self) -> Self {
        Self::Union(alloc::vec![self, Self::Basic(Basic::Nil)])
    }
}

/// An Elixir basic type.
///
/// Deliberately short. `pos_integer()`, `non_neg_integer()` and
/// `1..255` are all real Elixir types that a checker acts on, and none
/// of them is used: an IDL `octet` is not a `non_neg_integer()` — it is
/// eight bits — and a type that stated half the constraint would make
/// the other half look stated too. The width, the signedness and the
/// bound go in a module attribute, in one place, for every member.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Basic {
    /// `integer()`
    Integer,
    /// `float()`
    Float,
    /// `boolean()`
    Boolean,
    /// `binary()` — a run of octets, with no encoding of its own.
    Binary,
    /// `nil` — the one value, written without parentheses.
    Nil,
    /// `none()` — the type no value inhabits.
    ///
    /// What an enumeration with no enumerator this column emitted names.
    /// `atom()` would say every atom is one of them, which is the
    /// opposite of what an empty enumeration says.
    None,
    /// `any()`
    Any,
}

impl Basic {
    /// Every basic type the tree can hold.
    pub const ALL: [Self; 7] = [
        Self::Integer,
        Self::Float,
        Self::Boolean,
        Self::Binary,
        Self::Nil,
        Self::None,
        Self::Any,
    ];

    /// How it is written, parentheses included.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Integer => "integer()",
            Self::Float => "float()",
            Self::Boolean => "boolean()",
            Self::Binary => "binary()",
            // No parentheses: `nil` is a literal atom used as a type,
            // and `nil()` does not parse as one.
            Self::Nil => "nil",
            Self::None => "none()",
            Self::Any => "any()",
        }
    }
}

/// An expression, in the forms a generated module writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    /// A literal.
    Literal(Literal),
    /// `:name` — an atom.
    Atom(String),
    /// A variable, in a pattern or a body.
    Var(Ident),
    /// `@name` — reading a module attribute.
    Attr(Ident),
    /// A module name written as a value: `Common.Util.Location`.
    ///
    /// An Elixir alias in expression position evaluates to the atom
    /// `:"Elixir.Common.Util.Location"`, so this is how one generated
    /// declaration names another inside the metadata a codec reads back.
    /// Written as the alias rather than as that atom because the alias
    /// is what a reader recognises, and the two are the same value.
    Module(Path),
    /// `[a, b]`.
    List(Vec<Expr>),
    /// `{a, b}`.
    Tuple(Vec<Expr>),
    /// `[key: value, …]` — a keyword list.
    ///
    /// Its own form rather than a [`Expr::List`] of two-tuples, although
    /// that is exactly what it is: the keyword syntax is what a reader
    /// recognises and what `Keyword.get/2` is written against, and a
    /// tree that could only build the desugared form would produce
    /// output nobody writes by hand.
    Keyword(Vec<(Ident, Expr)>),

    // -- the codec layer's expression forms ---------------------------
    //
    // The forms below arrived with the codec backend
    // (`zerodds_lang_rules::emit::elixir`). The materialiser never
    // builds one; the declaration renderer still has to render one,
    // because a `defstruct` default and a codec value are one type.
    /// `Common.Mode.active(w, x)` — a call, remote or local by the
    /// path's length.
    Call {
        /// The function, outermost module segment first. One segment is
        /// a local call.
        func: Path,
        /// The arguments, in written order.
        args: Vec<Expr>,
    },
    /// `a op b`, with `op` already in Elixir's spelling — the backend
    /// maps `&&` to `and` and `&` to `&&&` before the tree holds it.
    Binary {
        /// The operator, as written.
        op: &'static str,
        /// The left operand.
        lhs: Box<Expr>,
        /// The right operand.
        rhs: Box<Expr>,
    },
    /// `op a` — `not a` is the only unary the codec writes; bitwise
    /// negation is spelled as the call `Bitwise.bnot(a)` because `~~~`
    /// is deprecated.
    Unary {
        /// The operator, `"not "` with its own trailing space.
        op: &'static str,
        /// The operand.
        value: Box<Expr>,
    },
    /// Parentheses written on purpose: the tree carries no precedence.
    Paren(Box<Expr>),
    /// `base.name` — a struct field read.
    Field {
        /// What is read into.
        value: Box<Expr>,
        /// The field.
        name: Ident,
    },
    /// One element of a list, rendered `Enum.at(value, index)`.
    ///
    /// Its own form and not a [`Expr::Call`], because the *write* side
    /// of the same place is a [`Statement::Update`] with an
    /// [`Access::At`] path — the two spellings of one place have to be
    /// recognisable as one, and a call to `Enum.at` would not be.
    Index {
        /// The list.
        value: Box<Expr>,
        /// The position, zero-based.
        index: Box<Expr>,
    },
    /// `if a, do: b, else: c` in expression position.
    IfElse {
        /// The condition.
        condition: Box<Expr>,
        /// The value when it holds.
        then: Box<Expr>,
        /// The value when it does not.
        otherwise: Box<Expr>,
    },
    /// `%Common.Pose{x: 0, y: 1}`.
    StructLiteral {
        /// The struct's module.
        module: Path,
        /// The fields written, in declaration order.
        fields: Vec<(Ident, Expr)>,
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

    /// `nil`.
    #[must_use]
    pub fn nil() -> Self {
        Self::Literal(Literal::Null)
    }
}

// -- the codec layer's statements -----------------------------------------
//
// Elixir has no statements: a `defmodule` body is expressions and a
// function body is one block expression. What it also does not have is
// mutation, and the shared codec walk in `zerodds_lang_rules::emit` is
// written against a target that does — it rebinds locals inside loop
// bodies and `if` branches and expects the binding to survive the block.
//
// This layer is the honest middle: a small imperative tree that *names*
// every binding it makes, so the renderer
// (`zerodds_lang_render::elixir::codec_function`) can compute which
// bindings each block changes and thread them through `if`/`Enum.reduce`
// as an accumulator tuple. The tree stays imperative and the lowering
// stays in one place, because a walk that had to answer "which variables
// does this block assign" itself would be answering a question about its
// own output.

/// One step into a value being rebuilt functionally.
///
/// The write-side counterpart of [`Expr::Field`] and [`Expr::Index`]:
/// a [`Statement::Update`] carries the path from the base variable to
/// the place, and the renderer spells it `put_in/3` with `Access.key!`
/// for a struct field and `Access.at` for a list element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Access {
    /// A struct field, by its atom.
    Key(Ident),
    /// A list element, by position.
    At(Expr),
}

/// One statement of a codec body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    /// `name = value`. Rebinding an existing name is deliberate and is
    /// what the renderer's threading is built on.
    Bind {
        /// The name bound. `_` discards.
        name: Ident,
        /// The value.
        value: Expr,
    },
    /// `{a, b} = value` — a destructuring bind, which is how a call
    /// that answers a value *and* a new writer or reader hands both
    /// back.
    BindTuple {
        /// The names, in tuple order. `_` discards a position.
        names: Vec<Ident>,
        /// The value.
        value: Expr,
    },
    /// `base = %{base | f: v}` or `base = put_in(base, [..], v)` — a
    /// write into a place inside `base`, which in Elixir is a rebind of
    /// `base` to the rebuilt value.
    Update {
        /// The variable holding the value written into.
        base: Ident,
        /// The path from `base` to the place.
        path: Vec<Access>,
        /// The value put there.
        value: Expr,
    },
    /// `if condition do … else … end`, with the branches' rebindings
    /// threaded out by the renderer.
    If {
        /// The condition, which must be pure: the emitter hoists any
        /// state-changing call out before building one of these.
        condition: Expr,
        /// The statements when it holds.
        then: Vec<Statement>,
        /// The statements when it does not. Empty renders no `else`
        /// unless the threading needs one.
        otherwise: Vec<Statement>,
    },
    /// A counted loop from zero to `bound`, exclusive, rendered as
    /// `Enum.reduce(0..(bound - 1)//1, acc, fn counter, acc -> … end)`.
    ///
    /// `//1` is load-bearing: `0..-1` without a step is a *descending*
    /// range in Elixir and would run a zero-bound loop twice.
    For {
        /// The counter's name, scoped to the body.
        counter: Ident,
        /// The exclusive bound.
        bound: Expr,
        /// The body.
        body: Vec<Statement>,
    },
    /// An uncounted loop, rendered as an anonymous function that calls
    /// itself while the condition holds. The condition must be pure for
    /// the reason [`Statement::If`]'s must, and unlike an `if` it is
    /// re-evaluated per pass, so the emitter refuses to hoist here.
    While {
        /// The condition.
        condition: Expr,
        /// The body.
        body: Vec<Statement>,
    },
    /// Leave the innermost enclosing loop: `throw({:zd_break, acc})`,
    /// caught at that loop's `try`.
    Break,
    /// Leave the whole routine with `value`:
    /// `throw({:zd_return, value})`, caught at the routine's `try`.
    Return(Expr),
    /// A comment on its own line.
    Comment(Doc),
    /// Statements standing where one statement was asked for.
    ///
    /// The emitter's escape for a seam that answers `Self::Stmt` and
    /// needs two — a hoisted reader call ahead of the `if` it feeds.
    /// The renderer flattens it; nothing nests meaning in it.
    Seq(Vec<Statement>),
}

/// A generated codec routine: `def name(params) do … result end`.
///
/// Not an [`Item::Function`], whose body is one expression by design —
/// see its documentation. A codec body is statements, and its `result`
/// is the expression the function answers with after them: the new
/// writer for an encoder, `{value, reader}` for a decoder, the value
/// itself for a Table 9 defaulter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodecFunction {
    /// The comment above it.
    pub doc: Doc,
    /// The name.
    pub name: Ident,
    /// The parameters, in order, as plain names.
    pub params: Vec<Ident>,
    /// The body.
    pub body: Vec<Statement>,
    /// What the routine answers with, after the body.
    pub result: Expr,
}

/// The atom an IDL identifier becomes.
///
/// Two steps, and the order is load-bearing.
///
/// 1. **The name is lowered.** An Elixir atom written without quotes and
///    a function name both have to start with a lower-case letter or an
///    underscore, so an IDL member `X` becomes `:x`. How much of the
///    name moves depends on whether it has a lower-case letter at all:
///
///    - **None** — `IDLE`, `MAX_SIZE`, `X` — and the whole name is
///      lowered: `idle`, `max_size`, `x`. An identifier written entirely
///      in upper case is one word to Elixir's eye, and `:iDLE` is not a
///      spelling of it.
///    - **Some** — `Pose`, `myField` — and only the first character
///      moves: `pose`, `myField`. Converting `myField` to `my_field` as
///      well would map two identifiers onto one atom, and IDL permits
///      both in one scope; an identifier that already has mixed case
///      says where its words are, so nothing has to be inferred.
///
///    Neither branch creates a collision the language did not already
///    have: IDL 4.2 §7.2.3 makes two identifiers that differ only in
///    case a collision in the IDL itself.
/// 2. **A reserved word takes a trailing underscore.** Lowering can
///    *create* one — `End` becomes `end` — which is why it runs first.
///
/// Neither step inverts, so the IDL spelling of every member is written
/// into the module's own attribute; see
/// `zerodds_lang_rules::adapter::elixir`.
///
/// The escape fires wherever a name is lowered, including where Elixir
/// would have accepted the reserved word. `defstruct end: 0` and
/// `%__MODULE__{end: integer()}` both compile (measured, Elixir 1.20.2),
/// and `def end, do: 0` is a syntax error — and an enumerator, a
/// constant and a bit each become both an atom and a function here, so
/// one IDL identifier has to have one Elixir spelling in both positions
/// or a codec reading the attribute could not find the function.
#[must_use]
pub fn atom_name(name: &str) -> String {
    escape_name(&lower_first(name))
}

/// The name a module segment takes.
///
/// An Elixir module name is an atom in the `Elixir.` namespace and each
/// of its segments must begin with an upper-case letter, so an IDL
/// module `common` becomes `Common`. Only the first character moves,
/// for the reason [`atom_name`] gives in the other direction.
///
/// A segment that names a module Elixir itself ships is refused by an
/// escape rather than written, because an Elixir alias is one flat atom
/// and a redefinition of one of those does not shadow — it *replaces*:
///
/// - **`Elixir`**, because `Elixir.S` and `S` are the *same* module.
///   `defmodule S` and `defmodule Elixir.S` in one file both produce
///   `Elixir.S.beam` and the second silently replaces the first, with no
///   diagnostic at all (measured, Elixir 1.20.2). An IDL
///   `module elixir { struct S; };` beside a top-level `struct S` would
///   otherwise lose one of the two.
/// - **`Kernel`**, because every module compiled after it in the same
///   file auto-imports it, and a `Kernel` without `def` is not a module
///   the rest of the file can be compiled against.
/// - **Every other module of the Elixir distribution**, because the
///   alias in the generated text resolves to the *preloaded* one.
///   Measured, Elixir 1.20.2: `defmodule Node do defstruct […] end`
///   followed in the same file by `%Node{}` is
///   `error: Node.__struct__/1 is undefined, cannot expand struct Node`
///   — the compiler answers with stdlib `Elixir.Node`, which has no
///   struct, and the whole file stops compiling. An IDL
///   `struct Node { sequence<Node> children; };` is an ordinary
///   recursive declaration. The names this column's own generated code
///   calls — `Enum`, `List`, `Access`, `String`, `Map` — are the same
///   hazard without the diagnostic: a declared `struct Enum` would take
///   over `Enum.reduce/3` for every module loaded in the same VM.
///
/// The escape is per segment rather than at the head only. `Common.Node`
/// could not collide, but a uniform rule needs no argument about when
/// `defmodule` nesting introduces an alias for its own last segment, and
/// the IDL spelling is in `@zd_idl` either way.
#[must_use]
pub fn module_segment(name: &str) -> String {
    let raised = upper_first(name);
    if RESERVED_MODULES.contains(&raised.as_str()) {
        let mut out = raised;
        out.push('_');
        return out;
    }
    raised
}

/// The name a module attribute takes.
///
/// [`atom_name`] first, and then a second escape for the attributes
/// Elixir reserves. `@doc 8` is not a compile error about a name — it is
/// `@doc is a built-in module attribute for documentation. It should be
/// either false, nil, a string, or a keyword list, got: 8` (measured,
/// Elixir 1.20.2) — and `@behaviour`, `@type` and `@compile` each fail
/// in their own way. An IDL `const long doc = 1;` is an ordinary
/// declaration, and it would take the whole generated file with it.
///
/// The one name this column's own output binds, `zd_constants`, is on
/// the list too. An IDL identifier may perfectly well be `zd_constants`,
/// and the second attribute of that name would silently replace the
/// first.
#[must_use]
pub fn attribute_name(name: &str) -> String {
    let lowered = atom_name(name);
    if RESERVED_ATTRIBUTES.contains(&lowered.as_str()) {
        let mut out = lowered;
        out.push('_');
        return out;
    }
    lowered
}

/// A trailing underscore where the name is one Elixir will not take.
///
/// Separate from [`atom_name`] so that a caller holding an
/// already-lowered name can escape it without lowering twice.
#[must_use]
pub fn escape_name(name: &str) -> String {
    if RESERVED.contains(&name) {
        let mut out = name.to_string();
        out.push('_');
        return out;
    }
    name.to_string()
}

/// The name lowered: all of it when it has no lower-case letter, and
/// otherwise its first character only.
fn lower_first(name: &str) -> String {
    if !name.chars().any(char::is_lowercase) {
        return name.to_lowercase();
    }
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut out = String::new();
    for lower in first.to_lowercase() {
        out.push(lower);
    }
    out.push_str(chars.as_str());
    out
}

/// The name with its first character raised.
fn upper_first(name: &str) -> String {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut out = String::new();
    for upper in first.to_uppercase() {
        out.push(upper);
    }
    out.push_str(chars.as_str());
    out
}

/// Every name a lowered identifier has to move out of the way of.
///
/// The first fifteen are Elixir's reserved words, which do not parse as
/// a function name at all. The last two are the functions this column's
/// enumeration modules define, which an enumerator of the same name
/// would redefine — Elixir would take the later clauses as further
/// clauses of the same function and warn about the grouping rather than
/// refuse, so one of the two meanings would be lost.
///
/// Elixir's own function names are **not** here, and that is measured
/// rather than assumed: `def node`, `def length`, `def if` and `def use`
/// all compile, because a local definition takes precedence over the
/// auto-imported `Kernel` one (Elixir 1.20.2). A generated module calls
/// nothing from `Kernel`, so shadowing one costs nothing, and escaping
/// them would rename members for no reason.
pub const RESERVED: &[&str] = &[
    // Elixir's reserved words.
    "after",
    "and",
    "catch",
    "do",
    "else",
    "end",
    "false",
    "fn",
    "in",
    "nil",
    "not",
    "or",
    "rescue",
    "true",
    "when", // The functions an enumeration module of this column defines.
    "value",
    "from_value",
];

/// Every module segment an escape has to move out of the way of.
///
/// `Elixir` and `Kernel` for the two reasons [`module_segment`] gives;
/// the rest is the single-segment module list of the Elixir
/// distribution, read off the code path of Elixir 1.20.2 /
/// Erlang/OTP 29 — `:code.all_available()`, the `Elixir.` prefix
/// stripped, the dotted names dropped. Multi-segment names (`Mix.Task`,
/// `ExUnit.Case`) are not here: their head segment is, and it is the
/// head that a generated `defmodule` would replace.
///
/// `ZeroddsWire` is on the list for the same reason and from the other
/// side: it is the head of this column's own wire runtime, which every
/// generated codec calls, and it is not declared in the generated file,
/// so the whole-file collision check cannot see it. An IDL
/// `struct ZeroddsWire` is a legal declaration.
pub const RESERVED_MODULES: &[&str] = &[
    "Access",
    "Agent",
    "Application",
    "ArgumentError",
    "ArithmeticError",
    "Atom",
    "BadArityError",
    "BadBooleanError",
    "BadFunctionError",
    "BadMapError",
    "Base",
    "Behaviour",
    "Bitwise",
    "Calendar",
    "CaseClauseError",
    "Code",
    "Collectable",
    "CompileError",
    "CondClauseError",
    "Config",
    "Date",
    "DateTime",
    "Dict",
    "Duration",
    "DynamicSupervisor",
    "EEx",
    "Elixir",
    "Enum",
    "Enumerable",
    "ErlangError",
    "ExUnit",
    "Exception",
    "File",
    "Float",
    "Function",
    "FunctionClauseError",
    "GenEvent",
    "GenServer",
    "HashDict",
    "HashSet",
    "IEx",
    "IO",
    "Inspect",
    "Integer",
    "JSON",
    "Kernel",
    "KeyError",
    "Keyword",
    "List",
    "Logger",
    "Macro",
    "Map",
    "MapSet",
    "MatchError",
    "MismatchedDelimiterError",
    "MissingApplicationsError",
    "Mix",
    "Module",
    "NaiveDateTime",
    "Node",
    "OptionParser",
    "PartitionSupervisor",
    "Path",
    "Port",
    "Process",
    "Protocol",
    "Range",
    "Record",
    "Regex",
    "Registry",
    "RuntimeError",
    "Set",
    "Stream",
    "String",
    "StringIO",
    "Supervisor",
    "SyntaxError",
    "System",
    "SystemLimitError",
    "Task",
    "Time",
    "TokenMissingError",
    "TryClauseError",
    "Tuple",
    "URI",
    "UndefinedFunctionError",
    "UnicodeConversionError",
    "Version",
    "WithClauseError",
    "ZeroddsWire",
];

/// Every module attribute Elixir reserves, and the one this column's own
/// output binds.
///
/// Elixir's own list is the `Module` documentation's "built-in
/// attributes", plus the typespec ones — an `@type` given an integer is
/// as broken as a `@doc` given one, and for a different reason.
pub const RESERVED_ATTRIBUTES: &[&str] = &[
    "after_compile",
    "before_compile",
    "behaviour",
    "callback",
    "compile",
    "deprecated",
    "derive",
    "dialyzer",
    "doc",
    "enforce_keys",
    "external_resource",
    "file",
    "impl",
    "macrocallback",
    "moduledoc",
    "nifs",
    "on_definition",
    "on_load",
    "opaque",
    "optional_callbacks",
    "spec",
    "type",
    "typedoc",
    "typep",
    "vsn",
    // This column's own.
    "zd_constants",
];

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;
    use alloc::format;

    #[test]
    fn every_basic_type_writes_its_own_spelling() {
        let mut seen = Vec::new();
        for basic in Basic::ALL {
            let text = basic.as_str();
            assert!(!seen.contains(&text), "{text} is used twice");
            seen.push(text);
        }
        assert_eq!(seen.len(), Basic::ALL.len());
    }

    #[test]
    fn nil_is_written_without_parentheses_and_the_rest_with_them() {
        // `nil` is a literal atom used as a type; `nil()` does not parse
        // as one.
        assert_eq!(Basic::Nil.as_str(), "nil");
        for basic in Basic::ALL {
            if basic != Basic::Nil {
                assert!(basic.as_str().ends_with("()"), "{}", basic.as_str());
            }
        }
    }

    #[test]
    fn an_identifier_with_mixed_case_lowers_its_first_character_only() {
        // `myField` keeps its later capitals, because `my_field` is a
        // legal IDL member of the same scope and mapping both onto one
        // atom would lose one.
        assert_eq!(atom_name("active"), "active");
        assert_eq!(atom_name("myField"), "myField");
        assert_eq!(atom_name("Pose"), "pose");
    }

    #[test]
    fn an_identifier_in_upper_case_throughout_is_lowered_throughout() {
        // An enumerator is conventionally written `IDLE`, and `:iDLE` is
        // not a spelling of it: with no lower-case letter in the name
        // there is nothing to say where the words are, so the whole name
        // is one.
        assert_eq!(atom_name("IDLE"), "idle");
        assert_eq!(atom_name("MAX_SIZE"), "max_size");
        assert_eq!(atom_name("X"), "x");
    }

    #[test]
    fn lowering_runs_before_the_escape_because_it_can_create_a_keyword() {
        // `End` lowers to `end`, which is a reserved word. Escaping
        // first would leave the escape nothing to do and produce `end`,
        // which is not a name a function definition can take.
        assert_eq!(atom_name("End"), "end_");
        assert_eq!(atom_name("end"), "end_");
        assert_eq!(atom_name("When"), "when_");
    }

    #[test]
    fn the_functions_an_enumeration_module_defines_are_escaped_too() {
        // An enumerator called `value` would otherwise define `value/0`
        // beside this column's `value/1`, and Elixir would read the two
        // as clauses of one function.
        assert_eq!(atom_name("VALUE"), "value_");
        assert_eq!(atom_name("from_value"), "from_value_");
    }

    #[test]
    fn a_name_elixir_merely_auto_imports_is_left_alone() {
        // Measured against Elixir 1.20.2: `def node`, `def length`,
        // `def if` and `def use` all compile, because a local definition
        // wins over the auto-imported `Kernel` one. A generated module
        // calls nothing from `Kernel`.
        for name in ["node", "self", "length", "if", "use", "raise"] {
            assert_eq!(atom_name(name), name);
        }
    }

    #[test]
    fn a_module_segment_raises_its_first_character() {
        assert_eq!(module_segment("common"), "Common");
        assert_eq!(module_segment("Location"), "Location");
        assert_eq!(module_segment("myModule"), "MyModule");
    }

    #[test]
    fn the_module_segment_elixir_is_escaped_because_it_is_the_root() {
        // `defmodule S` and `defmodule Elixir.S` are one module: both
        // produce `Elixir.S.beam` and the second replaces the first with
        // no diagnostic (measured, Elixir 1.20.2).
        assert_eq!(module_segment("elixir"), "Elixir_");
        assert_eq!(module_segment("Elixir"), "Elixir_");
        assert_eq!(module_segment("kernel"), "Kernel_");
    }

    #[test]
    fn a_module_segment_the_elixir_distribution_already_names_is_escaped() {
        // A redefinition of a distribution module does not shadow it, it
        // replaces it. Measured, Elixir 1.20.2: `defmodule Node do
        // defstruct […] end` and `%Node{}` in one file is `error:
        // Node.__struct__/1 is undefined, cannot expand struct Node` —
        // the alias answers with the preloaded stdlib module.
        assert_eq!(module_segment("Node"), "Node_");
        assert_eq!(module_segment("node"), "Node_");
        // The names the generated code itself calls.
        for name in ["Enum", "List", "Access", "String", "Map", "Range"] {
            assert_eq!(module_segment(name), format!("{name}_"));
        }
        // The wire runtime's own head segment, which the whole-file
        // collision check cannot see because nothing declares it there.
        assert_eq!(module_segment("ZeroddsWire"), "ZeroddsWire_");
        // Multi-segment distribution names are not on the list; their
        // head segment is, and a segment of the tail is an ordinary name.
        assert_eq!(module_segment("Task"), "Task_");
        assert_eq!(module_segment("Supervised"), "Supervised");
    }

    #[test]
    fn the_reserved_module_list_is_sorted_and_free_of_repeats() {
        // It is read off `:code.all_available()`; a name added by hand in
        // the wrong place is a name the next reader cannot find.
        for pair in RESERVED_MODULES.windows(2) {
            assert!(pair[0] < pair[1], "out of order: {} {}", pair[0], pair[1]);
        }
    }

    #[test]
    fn an_attribute_named_after_a_built_in_one_is_escaped() {
        // `@doc 8` is not a name error: Elixir reports "@doc is a
        // built-in module attribute for documentation. It should be
        // either false, nil, a string, or a keyword list, got: 8", and
        // the whole file stops compiling. An IDL `const long doc = 1;`
        // is an ordinary declaration.
        assert_eq!(attribute_name("doc"), "doc_");
        assert_eq!(attribute_name("BEHAVIOUR"), "behaviour_");
        assert_eq!(attribute_name("type"), "type_");
        assert_eq!(attribute_name("LIMIT"), "limit");
    }

    #[test]
    fn the_reserved_lists_have_no_duplicates() {
        for list in [RESERVED, RESERVED_MODULES, RESERVED_ATTRIBUTES] {
            let mut seen: Vec<&str> = Vec::new();
            for name in list {
                assert!(!seen.contains(name), "{name} is listed twice");
                seen.push(name);
            }
        }
    }

    #[test]
    fn every_reserved_word_escapes_to_something_that_is_not_reserved() {
        for name in RESERVED {
            let escaped = escape_name(name);
            assert_eq!(escaped, format!("{name}_"));
            assert!(!RESERVED.contains(&escaped.as_str()));
        }
    }

    #[test]
    fn a_module_name_keeps_its_segments_apart() {
        // `Common.Util.Location` is one module whose name has three
        // segments. The C and Go trees have to flatten a scope onto the
        // identifier; this one does not.
        let module = Module {
            doc: Doc::none(),
            name: Path::new(&["Common", "Util", "Location"]),
            body: Vec::new(),
        };
        assert_eq!(module.name.segments().len(), 3);
    }

    #[test]
    fn a_field_whose_type_has_no_zero_defaults_to_nil_and_keeps_its_place() {
        // Elixir's list syntax puts keyword pairs last, so a bare
        // `:pose` beside `x: 0` could only be written by moving it to
        // the front — and the field order is the IDL's. `nil` here and
        // the name in `@enforce_keys` says the same thing without
        // moving it.
        let field = Field {
            name: Ident::new("pose"),
            default: Expr::nil(),
        };
        assert_eq!(field.default, Expr::Literal(Literal::Null));
    }

    #[test]
    fn an_optional_type_is_the_type_or_nil() {
        let type_ = Type::Basic(Basic::Integer).or_nil();
        let Type::Union(arms) = &type_ else {
            panic!("not a union");
        };
        assert_eq!(arms.len(), 2);
        assert_eq!(arms[1], Type::Basic(Basic::Nil));
    }

    #[test]
    fn an_array_type_nests_outermost_dimension_first() {
        // `long grid[2][3]` is two rows of three, so the outer tuple has
        // two elements and each of them has three. Reversing it silently
        // transposes the data.
        let inner = Type::Tuple(alloc::vec![Type::Basic(Basic::Integer); 3]);
        let outer = Type::Tuple(alloc::vec![inner; 2]);
        let Type::Tuple(rows) = &outer else {
            panic!("not a tuple");
        };
        assert_eq!(rows.len(), 2);
        let Type::Tuple(columns) = &rows[0] else {
            panic!("the row is not a tuple");
        };
        assert_eq!(columns.len(), 3);
    }
}
