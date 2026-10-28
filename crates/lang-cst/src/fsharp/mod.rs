// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The F# concrete syntax tree. Spec §1.
//!
//! F# is read top to bottom, like Zig and Go, and a type reads left to
//! right and carries its own optionality and dimensionality — `Pose []`
//! is [`Type::Array`] holding [`Type::Named`], `Pose option` is
//! [`Type::Option`] holding it — so, as in [`crate::zig`] and
//! [`crate::go`], there is no declarator to split around a name. A field
//! is a name, a colon, and a type.
//!
//! ## Why a class and not a record
//!
//! F# has two aggregate forms and this tree uses neither the naive way.
//! A record (`type Pose = { X: int32; Y: float }`) is F#'s idiomatic
//! product type, and it is the wrong shape for a generated codec: a
//! record's fields — even `mutable` ones — must *all* be given at
//! construction (`{ X = 0; Y = 0.0 }`), so a decoder that builds one
//! member at a time, the way the shared codec walk builds every other
//! column's aggregate, would have nowhere to put the sample before every
//! field is known.
//!
//! [`ClassField`] is what a **class** gives instead: `member val X:
//! int32 = 0 with get, set` is a mutable, publicly settable property
//! with its own default, and `Pose()` constructs one with every field
//! already at DDS-XTypes 1.3 Table 9's value — measured (`fsc`
//! 9.0.106): a class with only `member val` properties compiles and
//! runs with no explicit constructor body at all. A decoder is then
//! `let v = Pose() in v.X <- ...; v.Y <- ...; v`, which is the same
//! declare-then-assign shape the walk builds for C, Java and Zig, and
//! [`ClassField::default`] is *always* written, because Table 9's value
//! is what a reader that skips a member leaves standing — there is no
//! separate generated defaulter function the way `runtime/zig` needs
//! one, because the language already has "the value this field takes
//! when nothing sets it" as a first-class thing to say.
//!
//! A record was tried in the reasoning that produced this tree and
//! rejected for exactly that gap: `runtime/fsharp` and
//! `crates/lang-rules/src/default_fsharp.rs` carry no defaulter-function
//! generator, which every value-shaped column before this one has
//! needed, because `T()` already answers Table 9 recursively — a class
//! held by a class field defaults to *its own* `member val` defaults in
//! turn.
//!
//! ## Why a union is a class too, and not a discriminated union
//!
//! F#'s discriminated union is the better F# form for a sum type, so —
//! as in [`crate::zig`]'s documentation of the same question for Zig's
//! tagged union — this is a decision that has to be argued.
//!
//! An IDL union's discriminator is a value with its own type
//! (IDL 4.2 §7.4.1.4.4.2.3), and it may take a value no `case` label
//! selects; when it does and the union declares no `default`, the union
//! carries **no** branch value at all and only the discriminator goes on
//! the wire (DDS-XTypes 1.3 §7.4.1.1.5.2). An F# discriminated union's
//! case *is* its tag — there is no state in which the tag holds a value
//! no case constructs — so the same gap Zig's `union(enum)` cannot cross
//! applies here unchanged. The class form used for a structure carries
//! it exactly: [`ClassField::default`] gives every branch field
//! (Table 9's value, recursively, exactly as an ordinary member's does)
//! and `_d` carries the discriminator's own value, whatever it is; the
//! codec writes only the branch the discriminator selects
//! (DDS-XTypes 1.3 §7.4.1.1.5.2), which is the codec's decision and not
//! the type's.
//!
//! ## No pointer, because F# does not need one
//!
//! `@external` (DDS-XTypes 1.3 Table 21) says a member is stored outside
//! the containing object; Zig answers with `*T` because a Zig struct is
//! a value and a self-recursive one has no size without the
//! indirection. An F# class is a reference type already — `Node()`
//! allocates on the heap and `Node option` inside `Node` is exactly as
//! finite as it is in Java or C# — so `@external` changes nothing this
//! tree has to hold apart from the plain member. [`crate::d`]'s `T*` and
//! this tree's absence of one are the same finding read two ways: an
//! indirection is only a tree concern where the language's default
//! aggregate is a value.
//!
//! ## No `Slice` beside `Array`
//!
//! Zig's tree keeps `[]T` and `[N]T` apart because Zig's fixed-length
//! array is a distinct type from its slice. .NET has one array type
//! either way — `T[]` carries no compile-time length — so a
//! `sequence<T>` and a `T[N]` are the same [`Type::Array`], and `T[N][M]`
//! is `Array(Array(T))`, outer dimension outermost, exactly as
//! [`crate::zig`]'s nesting is. The length a fixed array holds to is
//! enforced by the generated codec, which is where C++'s `std::array`
//! bound and Java's fixed-length check live too — never by the type.
//!
//! ## `long double` has no built-in type here, and a real wire form
//! ## anyway
//!
//! .NET's floating-point set is `float32` and `float` (`System.Double`,
//! 64 bits) and nothing wider; there is no 128-bit float the way Zig's
//! `f128` is one. This tree holds an IDL `long double` as
//! [`Type::Primitive`]`(`[`Primitive::String`]`)`, the decimal text, the
//! same storage [`crate::go`] and [`crate::python`] give for the same
//! reason — a workaround Zig's own documentation says is *not* needed
//! there and *is* needed everywhere the built-in set stops at binary64.
//!
//! The octets are not a workaround: `runtime/fsharp/ZdWire.fs` writes
//! and reads a real sixteen-byte IEEE 754 binary128 pattern for it, on
//! `System.Numerics.BigInteger`, ported line for line from
//! `runtime/go/float128.go` — the same algorithm three other columns
//! already carry, not a second one to disagree with them. What the
//! *tree* cannot do is give the value a native F# numeric type to live
//! in between the wire and the member; the text is where it lives
//! instead, exactly as it does in Go.
//!
//! ## The statement layer
//!
//! [`Stmt`] carries the forms the shared codec walk emits and two that
//! are F#'s own:
//!
//! - [`Stmt::CountUp`] renders `for i = 0 to bound - 1 do …`, F#'s own
//!   counted loop, whose counter is scoped to the loop by the language —
//!   unlike Zig, which has no C-style `for` and wraps a `while` in a
//!   block for the same scoping. Measured: two sequential `for i = ...`
//!   loops with the same counter name compile with no shadowing error
//!   (`fsc` 9.0.106).
//! - [`Stmt::Until`]/[`Stmt::Leave`] answer a loop with no bound — a
//!   `@mutable` aggregate's headers, a PL_CDR1 list's sentinel — where
//!   F# has nothing resembling C's or Zig's `break`. The renderer wraps
//!   the loop in a `try … with :? ZdWire.LoopBreak -> ()` and
//!   [`Stmt::Leave`] is `raise (ZdWire.LoopBreak())`: the exception
//!   unwinds exactly one enclosing `try`, which is "leave the enclosing
//!   loop" read through the one control-transfer F# has for it. No
//!   coordination between the two is needed — neither carries a name the
//!   other must agree on — which is not true of a hand-rolled mutable
//!   flag and is why this is the shape and not that one.
//! - [`Stmt::GiveBack`]/[`Stmt::Guarded`] are the same pair one level
//!   out, for a `return` the shared walk writes inside a loop body:
//!   `raise (ZdWire.GiveBack())` unwinds past every `Until`'s `try`,
//!   because an F# exception filter matches on the exception's type.
//!
//! Every routine result is written [`Stmt::Return`]: F# has no `return`
//! keyword and a function's value is its body's last expression, so this
//! variant is not F#'s `return` — it is "the trailing expression", and
//! the renderer writes its expression bare with no keyword. It must be
//! the body's last statement; nothing after it is reachable and the
//! renderer does not check, because the walk that builds a body never
//! writes one anywhere else.
//!
//! ## Mutual recursion needs one word, not a forward declaration
//!
//! `module rec` — `File::module_` is always rendered as `module rec
//! <name>` — makes every declaration in the file visible to every other
//! one regardless of order, which is what a self-recursive or
//! mutually-recursive pair of generated types and their codecs need.
//! Measured (`fsc` 9.0.106): a class referencing another declared after
//! it, and a `let` function calling one declared after it, both compile
//! under `module rec` and neither does without it. This is F#'s reading
//! of the same fact Zig's module documentation states for its own
//! container ("Zig resolves a declaration anywhere in the file's
//! container regardless of where it is written") — one keyword rather
//! than a property the language already has.
//!
//! ## Escaping reaches further than it does in Zig
//!
//! Zig's field names live in a namespace the language's own primitive
//! and keyword names are not part of, so [`crate::zig::escape_field`]
//! only ever escapes a *keyword*. F# has no such carve-out: measured,
//! `member val type: int32 = 0 with get, set` is `error FS0010:
//! Unexpected keyword 'type' in member definition` — the parser refuses
//! it in a field position exactly as it would at the top level. There is
//! therefore one escape list here, [`KEYWORDS`], checked in both
//! positions [`escape_field`] and [`escape_decl`] answer, with
//! [`escape_decl`] additionally checking [`PRIMITIVE_NAMES`] so a
//! declaration cannot collide with a built-in type abbreviation such as
//! `byte` or `int`. As in Zig, the escape is a trailing underscore and
//! not F#'s `` `…` `` arbitrary-identifier syntax — the same two reasons
//! apply unchanged: an IDL identifier
//! (`[A-Za-z_][A-Za-z0-9_]*`, IDL 4.2 §7.2.3) never needs the wider form,
//! and a field named `` `type` `` reads like an escape hatch where
//! `type_` reads like a name.

use crate::common::{BodyText, Doc, Ident, Literal};
use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// An F# source file: one `module rec` per IDL file.
///
/// One file per IDL file, as in Zig, Go and C: F#'s unit of mutual
/// visibility is the module, and nested IDL modules flatten onto the
/// declared names for the same reason those columns give — a file does
/// not nest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct File {
    /// The container documentation, written above the `module rec` line.
    pub doc: Doc,
    /// The module's own name.
    pub module: Ident,
    /// Declarations, in written order.
    pub decls: Vec<Decl>,
}

/// One top-level declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decl {
    /// A type: a class, an alias or an enum.
    Type(TypeDecl),
    /// A `let` binding: a codec routine, or a module-level constant.
    Let(LetDecl),
    /// A comment standing on its own.
    Comment(Doc),
    /// `@verbatim` text, copied into the output unchanged — OMG IDL 4.2
    /// §8.3.5.1 asks for the text "verbatim", so raw lines and never a
    /// comment.
    Verbatim(String),
}

/// `type Name = …`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeDecl {
    /// Documentation, written as `///`.
    pub doc: Doc,
    /// The name declared.
    pub name: Ident,
    /// What it declares.
    pub body: TypeBody,
    /// `@verbatim` text at the start and the end of a class's body.
    /// Empty for any other body.
    pub body_text: BodyText,
}

/// The right-hand side of a type declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeBody {
    /// `type Name = <aliased>` — a bitmask's holder, a typedef's target,
    /// a `native` declaration's mapping. F# has no separate "new type"
    /// form for this the way Zig's `pub const` also does not: the
    /// right-hand side is a type and the declaration binds a synonym.
    Alias(Type),
    /// `type Name() = member val … with get, set` — the mutable class
    /// every aggregate becomes. See the module documentation for why a
    /// class and not a record.
    Class(Vec<ClassField>),
    /// `type Name = | A = 0 | B = 4` — a CLI enum with an explicit
    /// backing type and every value written out, exactly as Zig's
    /// `enum(T) { … }` writes every value: F# would number an unvalued
    /// case consecutively from zero and `@value` admits gaps.
    Enum {
        /// The backing integer type. DDS-XTypes 1.3 §7.4.3.5.3 makes an
        /// enumeration's tag signed; the wire width is the
        /// `@bit_bound`'s and is the runtime's concern.
        backing: Type,
        /// The cases, in written order.
        variants: Vec<Variant>,
    },
}

/// One `member val` property of a class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassField {
    /// Documentation.
    pub doc: Doc,
    /// The property's name, as it will be written.
    pub name: Ident,
    /// The type.
    pub type_: Type,
    /// The value the class's own constructor gives it.
    ///
    /// Always present and never absent the way [`crate::zig::Field`]'s
    /// default is: a `member val` needs an initialiser to parse at all,
    /// and DDS-XTypes 1.3 Table 9 always has an answer, so there is
    /// nothing this position can be empty for.
    pub default: Expr,
}

/// One case of an enumeration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variant {
    /// Documentation.
    pub doc: Doc,
    /// The name.
    pub name: Ident,
    /// `= value`, always written. See [`TypeBody::Enum`].
    pub value: Expr,
}

/// `let name (p1: T1) (p2: T2) : R = body`, or `let name : T = value`
/// for a parameterless binding — a module-level constant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LetDecl {
    /// Documentation.
    pub doc: Doc,
    /// The name declared.
    pub name: Ident,
    /// Parameters, in order. Empty for a constant.
    pub params: Vec<FnParam>,
    /// The result type. Written for a function; for a constant this is
    /// the ascription `let NAME : T = value` carries.
    pub type_: Option<Type>,
    /// The body. A constant's is one [`Stmt::Return`].
    pub body: Vec<Stmt>,
}

/// One parameter of a `let` function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FnParam {
    /// The name.
    pub name: Ident,
    /// The type.
    pub type_: Type,
}

/// An F# type, as it will be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    /// A built-in.
    Primitive(Primitive),
    /// A declared name, or a runtime type qualified with its module.
    Named(Ident),
    /// `T []` — a `sequence<T>` and a `T[N]` alike; see the module
    /// documentation for why there is one array shape and not two.
    Array(Box<Type>),
    /// `'T option` — an `@optional` member, `None` when absent.
    Option(Box<Type>),
    /// `T1 -> T2 -> R` — an interface operation held as a function
    /// value, curried, F#'s own function-type spelling.
    Fn {
        /// Parameter types, in order.
        params: Vec<Type>,
        /// The result. `unit` for an operation with nothing to answer.
        returns: Box<Type>,
    },
}

impl Type {
    /// A type spelled by one name.
    #[must_use]
    pub fn named(text: &str) -> Self {
        Self::Named(Ident::new(text))
    }

    /// `T []`.
    #[must_use]
    pub fn array(element: Self) -> Self {
        Self::Array(Box::new(element))
    }

    /// `'T option`.
    #[must_use]
    pub fn option(inner: Self) -> Self {
        Self::Option(Box::new(inner))
    }

    /// `string` — the shape a `string`, a `wstring` and a `fixed<P,S>`
    /// all take. .NET's `string` is a real Unicode text type, so unlike
    /// Zig's `[]const u8` workaround nothing here re-derives an
    /// encoding; the runtime converts to and from UTF-8 octets at the
    /// wire boundary and this tree never has to say so.
    #[must_use]
    pub fn text() -> Self {
        Self::Primitive(Primitive::String)
    }
}

/// An F# built-in type, by its lower-case alias.
///
/// `nativeint`, `unativeint` and `char` (.NET's UTF-16 code unit) are
/// **not** here: no IDL primitive reaches a pointer-width integer, and
/// an IDL `char`/`wchar` is a code point held in a fixed-width integer —
/// `crate::adapter::fsharp` carries the table — never .NET's own
/// `char`, whose width follows neither.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Primitive {
    /// `bool`
    Bool,
    /// `sbyte`
    SByte,
    /// `byte`
    Byte,
    /// `int16`
    Int16,
    /// `uint16`
    UInt16,
    /// `int32`
    Int32,
    /// `uint32`
    UInt32,
    /// `int64`
    Int64,
    /// `uint64`
    UInt64,
    /// `float32`
    Float32,
    /// `float` — .NET's `System.Double`.
    Double,
    /// `string` — also `long double`'s decimal text; the module
    /// documentation says why there is no wider float here.
    String,
    /// `unit` — what a codec routine that writes returns.
    Unit,
}

impl Primitive {
    /// Every built-in the tree can hold.
    pub const ALL: [Self; 12] = [
        Self::Bool,
        Self::SByte,
        Self::Byte,
        Self::Int16,
        Self::UInt16,
        Self::Int32,
        Self::UInt32,
        Self::Int64,
        Self::UInt64,
        Self::Float32,
        Self::Double,
        Self::String,
    ];

    /// How it is written.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::SByte => "sbyte",
            Self::Byte => "byte",
            Self::Int16 => "int16",
            Self::UInt16 => "uint16",
            Self::Int32 => "int32",
            Self::UInt32 => "uint32",
            Self::Int64 => "int64",
            Self::UInt64 => "uint64",
            Self::Float32 => "float32",
            Self::Double => "float",
            Self::String => "string",
            Self::Unit => "unit",
        }
    }
}

/// A statement, as a codec body writes one.
///
/// Only the forms the shared codec walk emits, plus F#'s own answers to
/// a bound loop, an unbound one and a routine's result — the module
/// documentation carries why each is shaped the way it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    /// `let [mutable] name [: T] = value`.
    Let {
        /// The name declared.
        name: Ident,
        /// The declared type; absent lets the initialiser carry it.
        type_: Option<Type>,
        /// The initialiser.
        value: Expr,
        /// Whether a later statement rebinds it with `<-`. F# refuses
        /// `<-` against a non-`mutable` `let` ("this value is not
        /// mutable"), so a local the body only ever reads is `let` and
        /// one it reassigns is `let mutable` — the emitter decides from
        /// what the body does, the same question Zig's `var`/`const`
        /// choice answers for the same reason.
        mutable: bool,
    },
    /// `place <- value` — reassigning a `let mutable` local, or setting
    /// a mutable property or an array element, neither of which needs
    /// the binding itself to be mutable.
    Assign {
        /// The place written.
        place: Expr,
        /// The value.
        value: Expr,
    },
    /// An expression evaluated for its effect; F# gives a discarded
    /// non-`unit` value a warning and not an error, but the walk never
    /// hands this a value worth keeping, so nothing here forces one.
    Expr(Expr),
    /// `if condition then … else …`, written as a statement — every
    /// branch is `unit`-typed, which every arm the walk builds already
    /// is.
    If {
        /// The condition.
        condition: Expr,
        /// The then-branch.
        then: Vec<Stmt>,
        /// The else-branch, absent where there is none.
        otherwise: Option<Vec<Stmt>>,
    },
    /// `for counter = 0 to bound - 1 do …` — F#'s own counted loop; see
    /// the module documentation for why this needs no scoping trick the
    /// way Zig's does.
    CountUp {
        /// The counter's name.
        counter: Ident,
        /// The counter's type.
        type_: Box<Type>,
        /// The exclusive bound.
        bound: Box<Expr>,
        /// The body.
        body: Vec<Stmt>,
    },
    /// A loop with no counter, closed by [`Stmt::Leave`] and not by a
    /// bound. Rendered as `try (while condition do …) with :?
    /// ZdWire.LoopBreak -> ()`; see the module documentation.
    Until {
        /// The condition.
        condition: Expr,
        /// The body.
        body: Vec<Stmt>,
    },
    /// `raise (ZdWire.LoopBreak())` — F#'s answer to a bare `break`; the
    /// module documentation says why an exception and not a flag.
    Leave,
    /// `raise (ZdWire.GiveBack())` — F#'s answer to a `return` that is
    /// not the body's last statement.
    ///
    /// [`Stmt::Return`] is the trailing expression and can only be
    /// written where a trailing expression goes.
    /// `crates/lang-rules/src/emit/fsharp.rs`'s `hoist_early_returns`
    /// moves most of the shared walk's early returns there by pushing
    /// what followed them down into the branches, but one shape it
    /// cannot reach: a return inside a **loop body**, which the
    /// `@mutable` read walk emits for a second header carrying an id
    /// already read (DDS-XTypes 1.3 §7.4.3.4.2). Pushing the function's
    /// tail into a loop body would run it once per iteration.
    ///
    /// So the same answer [`Stmt::Leave`] gives for `break`, one `try`
    /// further out: [`Stmt::Guarded`] wraps the body and this raises
    /// past every enclosing `Until` — an exception filter matches by
    /// type, so `with :? ZdWire.LoopBreak` does not catch it.
    GiveBack,
    /// `try … with :? ZdWire.GiveBack -> ()` around a routine's body,
    /// with the routine's trailing expression after it.
    ///
    /// Written by `crates/lang-rules/src/emit/fsharp.rs`'s `routine` and
    /// only there, and only when the body holds a [`Stmt::GiveBack`] —
    /// a `try` around a body that never raises would be noise in every
    /// other decoder in the file.
    Guarded {
        /// The statements the guard covers.
        body: Vec<Stmt>,
    },
    /// A comment on its own line.
    Comment(Doc),
    /// The body's trailing expression — not F#'s `return` keyword,
    /// which does not exist; see the module documentation.
    Return(Expr),
}

/// An expression.
///
/// The forms the codec walk's statement layer needs, plus `Some`/`None`
/// for `@optional`, plain construction for a decoder's starting value,
/// and object-initialiser construction for the one place a whole value
/// is built as a single expression rather than assigned into
/// field by field — `runtime/fsharp`'s and the generator's `fill`
/// module build a sample this way, the same job Zig's anonymous
/// `.{ … }` struct literal does.
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
    /// `(a)` — parentheses written on purpose; the tree carries no
    /// operator precedence.
    Paren(Box<Expr>),
    /// `f(a, b)` — a call in .NET's own tupled convention: what a
    /// method on the runtime's `Writer`/`Reader` is called with,
    /// `w.WriteU32(x)`. Not what this column's own generated `let`
    /// functions want — see [`Expr::CurriedCall`] and its documentation
    /// for why the two are not interchangeable.
    Call {
        /// What is called.
        func: Box<Expr>,
        /// The arguments, in order.
        args: Vec<Expr>,
    },
    /// `f a b` — a call in F#'s native curried convention: every
    /// generated `let` function this column writes — a codec routine,
    /// a defaulter, a free function of `runtime/fsharp/ZdWire.fs` — is
    /// one, and calling a two-parameter curried function as `f(a, b)`
    /// does not compile: `(a, b)` constructs one tuple value and a
    /// curried function wants two arguments, not one. Measured
    /// (`fsc` 9.0.106): `let f (a: int) (b: int) = a + b` called as
    /// `f(1, 2)` is `error FS0001`, "This expression was expected to
    /// have type 'int' … 'int * int'". Every argument is parenthesised
    /// on its own, which is always unambiguous regardless of what the
    /// argument expression is.
    CurriedCall {
        /// What is called.
        func: Box<Expr>,
        /// The arguments, in order.
        args: Vec<Expr>,
    },
    /// `value.name` — a property access, a module-qualified name, or a
    /// runtime member reached through its receiver.
    Dot {
        /// The value accessed.
        value: Box<Expr>,
        /// The name accessed.
        name: Ident,
    },
    /// `value.[index]` — F#'s array/list index syntax.
    Index {
        /// The value indexed.
        value: Box<Expr>,
        /// The index.
        index: Box<Expr>,
    },
    /// `op value` — `not`, `-`, written as given with the space its
    /// operator needs.
    Unary {
        /// The operator, as written.
        op: &'static str,
        /// The operand.
        value: Box<Expr>,
    },
    /// `Some value`.
    Some_(Box<Expr>),
    /// `None`.
    None_,
    /// `Type(args…)` — plain positional construction: `Pose()` for a
    /// decoder's defaulted starting value, or a runtime type built from
    /// positional arguments.
    New {
        /// The type constructed.
        type_: Ident,
        /// The constructor's positional arguments.
        args: Vec<Expr>,
    },
    /// `Type(Field1 = e1, Field2 = e2)` — object-initialiser
    /// construction, one expression for a whole value. What `fill`
    /// builds a sample with, and the union counterpart of a structure's:
    /// every field is named, so a branch the sample does not select
    /// still needs the value its own type defaults to and never an
    /// uninitialised one.
    ObjectInit {
        /// The type constructed.
        type_: Ident,
        /// The properties set, in order.
        fields: Vec<(Ident, Expr)>,
    },
    /// `[| a; b; c |]`, or `[||]` for an empty array — Table 9's "a
    /// zero-length sequence of the same element type" and `fill`'s
    /// literal for a `sequence<T>` or a `T[N]` sample alike.
    ArrayLit(Vec<Expr>),
    /// `Array.zeroCreate<T> (n)` — the one place a bare type has to be
    /// written inside an expression rather than at a declaration: a
    /// reader's `make_room` needs an array of `T`'s own zero value —
    /// `null` for a class, which the fill that follows overwrites slot
    /// by slot. A dedicated variant and not a general "type as
    /// expression" the way Zig's `Expr::Type` is one, because this is
    /// the only call this column's codec walk ever needs a type
    /// argument written into.
    ArrayZeroCreate {
        /// The element type.
        element: Box<Type>,
        /// The element count.
        count: Box<Expr>,
    },
}

impl Expr {
    /// An integer literal.
    #[must_use]
    pub fn int(value: i128) -> Self {
        Self::Literal(Literal::int(value))
    }

    /// `value.name`.
    #[must_use]
    pub fn dot(value: Self, name: &str) -> Self {
        Self::Dot {
            value: Box::new(value),
            name: Ident::new(name),
        }
    }

    /// `f(args)` on a bare name.
    #[must_use]
    pub fn call(callee: &str, args: Vec<Self>) -> Self {
        Self::Call {
            func: Box::new(Self::Name(Ident::new(callee))),
            args,
        }
    }

    /// `f a b` on a bare or module-qualified name.
    #[must_use]
    pub fn curried_call(callee: &str, args: Vec<Self>) -> Self {
        Self::CurriedCall {
            func: Box::new(Self::Name(Ident::new(callee))),
            args,
        }
    }

    /// `Type()` or `Type(args)`.
    #[must_use]
    pub fn new(type_: &str, args: Vec<Self>) -> Self {
        Self::New {
            type_: Ident::new(type_),
            args,
        }
    }
}

/// The name an IDL identifier takes as a **member** of a class or a
/// case of an enumeration.
///
/// A trailing underscore where the identifier is an F# keyword, and
/// nowhere else. Unlike [`crate::zig::escape_field`], this is not a
/// narrower list than the declaration escape: measured (`fsc` 9.0.106),
/// `member val type: int32 = 0 with get, set` is
/// `error FS0010: Unexpected keyword 'type' in member definition` — a
/// keyword is refused in a member position exactly as it is at the top
/// level, so both escapes read the same list.
#[must_use]
pub fn escape_field(name: &str) -> String {
    if KEYWORDS.contains(&name) {
        let mut out = name.to_string();
        out.push('_');
        return out;
    }
    name.to_string()
}

/// The name an IDL identifier takes as a **declaration** in a file.
///
/// A trailing underscore for an F# keyword, as [`escape_field`] does,
/// and additionally for every name [`PRIMITIVE_NAMES`] lists — a
/// declaration is not refused the way [`crate::zig::escape_decl`]'s
/// primitive collision is (`type byte = …` shadows the built-in
/// alias rather than being refused outright), but the escape is applied
/// anyway rather than relied on to compile correctly: a generated
/// `ByteWriter` calling a member's own `byte` type would be reading a
/// declaration that shadowed the primitive it meant, and nothing in the
/// output would say so.
#[must_use]
pub fn escape_decl(name: &str) -> String {
    if KEYWORDS.contains(&name) || PRIMITIVE_NAMES.contains(&name) {
        let mut out = name.to_string();
        out.push('_');
        return out;
    }
    name.to_string()
}

/// F#'s keywords, reserved and reserved-for-future-use alike (F#
/// language reference, "Keyword Reference"). A keyword cannot be an
/// identifier anywhere without `` `…` ``, which this column does not
/// use — the module documentation says why.
pub const KEYWORDS: &[&str] = &[
    "abstract",
    "and",
    "as",
    "assert",
    "atomic",
    "base",
    "begin",
    "break",
    "checked",
    "class",
    "component",
    "const",
    "constraint",
    "constructor",
    "continue",
    "default",
    "delegate",
    "do",
    "done",
    "downcast",
    "downto",
    "eager",
    "elif",
    "else",
    "end",
    "event",
    "exception",
    "extern",
    "external",
    "false",
    "finally",
    "fixed",
    "for",
    "fun",
    "function",
    "functor",
    "global",
    "if",
    "in",
    "include",
    "inherit",
    "inline",
    "interface",
    "internal",
    "lazy",
    "let",
    "match",
    "member",
    "method",
    "mixin",
    "module",
    "mutable",
    "namespace",
    "new",
    "null",
    "object",
    "of",
    "open",
    "or",
    "override",
    "parallel",
    "private",
    "process",
    "protected",
    "public",
    "pure",
    "rec",
    "return",
    "sealed",
    "sig",
    "static",
    "struct",
    "tailcall",
    "then",
    "to",
    "trait",
    "true",
    "try",
    "type",
    "upcast",
    "use",
    "val",
    "virtual",
    "void",
    "volatile",
    "when",
    "while",
    "with",
    "yield",
];

/// F#'s built-in type abbreviations that [`escape_decl`] refuses to let
/// a declaration collide with, whether or not this tree ever emits the
/// type itself. Not keywords — the parser reads them as ordinary
/// identifiers — but reserved by [`escape_decl`] all the same, for the
/// reason its own documentation gives.
pub const PRIMITIVE_NAMES: &[&str] = &[
    "bool",
    "sbyte",
    "byte",
    "int16",
    "uint16",
    "int32",
    "uint32",
    "int64",
    "uint64",
    "int",
    "uint",
    "float32",
    "single",
    "float",
    "double",
    "string",
    "unit",
    "obj",
    "char",
    "decimal",
    "nativeint",
    "unativeint",
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
    fn there_is_no_binary128_because_dotnet_has_none() {
        // The one place this column differs from Zig's: `long double`
        // is text here, as it is in Go and Python, because .NET's
        // widest float is `System.Double`.
        assert!(!Primitive::ALL.contains(&Primitive::Unit));
        for primitive in Primitive::ALL {
            assert_ne!(primitive.as_str(), "f128");
        }
    }

    #[test]
    fn a_keyword_escapes_in_both_positions() {
        for name in ["type", "member", "let", "match", "mutable"] {
            assert_eq!(escape_field(name), alloc::format!("{name}_"));
            assert_eq!(escape_decl(name), alloc::format!("{name}_"));
        }
    }

    #[test]
    fn a_primitive_name_escapes_as_a_declaration_and_not_as_a_field() {
        // Unlike Zig, this is a conservative choice and not a measured
        // refusal — the module documentation says so.
        for name in ["byte", "int", "string", "unit"] {
            assert_eq!(escape_field(name), name);
            assert_eq!(escape_decl(name), alloc::format!("{name}_"));
        }
    }

    #[test]
    fn a_plain_name_is_left_exactly_as_the_idl_wrote_it() {
        for name in ["Pose", "pose", "x_min", "_d", "MODULE_ID"] {
            assert_eq!(escape_field(name), name);
            assert_eq!(escape_decl(name), name);
        }
    }

    #[test]
    fn neither_reserved_list_repeats_itself() {
        for list in [KEYWORDS, PRIMITIVE_NAMES] {
            let mut seen: Vec<&str> = Vec::new();
            for name in list {
                assert!(!seen.contains(name), "{name} is listed twice");
                seen.push(name);
            }
        }
    }

    #[test]
    fn an_array_type_nests_outermost_dimension_first() {
        // `long grid[2][3]` is two rows of three, which is `int32[][]`
        // with the outer array holding the rows.
        let type_ = Type::array(Type::array(Type::Primitive(Primitive::Int32)));
        let Type::Array(outer) = &type_ else {
            panic!("not an array");
        };
        assert!(matches!(**outer, Type::Array(_)));
    }

    #[test]
    fn text_is_the_dotnet_string_type() {
        let Type::Primitive(primitive) = Type::text() else {
            panic!("not a primitive");
        };
        assert_eq!(primitive, Primitive::String);
    }

    #[test]
    fn a_class_field_always_carries_a_default() {
        // Unlike Zig's `Field::default`, which is `Option` because only
        // an `@optional` member has one, this is never absent: the
        // module documentation says why a class needs one at every
        // field to construct at all.
        let field = ClassField {
            doc: Doc::none(),
            name: Ident::new("x"),
            type_: Type::option(Type::Primitive(Primitive::Byte)),
            default: Expr::None_,
        };
        assert_eq!(field.default, Expr::None_);
    }
}
