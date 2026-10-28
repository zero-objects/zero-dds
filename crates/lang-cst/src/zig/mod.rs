// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Zig concrete syntax tree. Spec §1.
//!
//! Zig has one declaration form. `pub const Pose = struct { … };` and
//! `pub const LIMIT: u8 = 8;` are the same production with a different
//! right-hand side, because a type in Zig is an ordinary comptime value
//! (Zig language reference, "comptime"). The tree says so: there is one
//! [`ConstDecl`], and [`Expr::Type`] is what lets its value be a type.
//! A tree with a separate `TypeDecl` would be inventing a distinction
//! the language does not have, and the renderer would then be the place
//! where the two were put back together.
//!
//! A type also reads left to right and carries its own length, pointer
//! depth and optionality, so — as in [`crate::go`] and unlike
//! [`crate::c`] — there is no declarator to split around a name.
//! `[2][3]i32` is an [`Type::Array`] holding an [`Type::Array`] holding
//! a [`Type::Primitive`].
//!
//! ## Three things this tree holds that no sibling's does
//!
//! 1. **[`Primitive::F128`].** Zig has a real binary128, `f128`, and it
//!    is the arithmetic type the language's own `comptime_float` is
//!    evaluated in. Of the ten language columns it is the only one whose
//!    built-in floating-point set reaches an IDL `long double`'s 113
//!    bits of significand; Go and Python have to hold the decimal in a
//!    string instead, and say so in their own documentation.
//! 2. **A field default.** Zig struct fields may carry one
//!    ([`Field::default`]), which is how an `@optional` member gets
//!    `null` as its resting state rather than needing a constructor to
//!    put it there.
//! 3. **[`AnalyseAll`].** Zig analyses container-level declarations
//!    lazily: `pub const X: u8 = 300;` alone in a file passes
//!    `zig build-obj` without a word, because nothing referenced `X`.
//!    A generated file of types that nothing in the compilation
//!    references is therefore never checked at all. [`AnalyseAll`] is
//!    the one `test` block that references every declaration, so that
//!    `zig test` over the generated file is a real semantic check and
//!    not a parse.
//!
//! ## No visibility field
//!
//! Every declaration is `pub`. Zig's `pub` controls whether an importer
//! of the file can name a declaration (Zig language reference, "import"),
//! and a generated types file exists to be imported; a private
//! declaration in one would have no consumer. [`ConstDecl::is_pub`] is
//! still a field rather than an assumption, because a renderer that
//! wrote `pub` unconditionally would be deciding something.
//!
//! ## Why `@"…"` is not in this tree
//!
//! Zig can write any string as an identifier: `@"align"` and
//! `@"hello world"` are both legal names. That is not what
//! [`escape_field`] and [`escape_decl`] do. They append an underscore,
//! for two reasons.
//!
//! The first is that `@"…"` accepts strings that are not identifiers at
//! all, so a column that used it would keep compiling if a name arrived
//! that no language could hold — the escape would stop being visible as
//! an escape. An IDL identifier is `[A-Za-z_][A-Za-z0-9_]*`
//! (IDL 4.2 §7.2.3) and is always a Zig identifier once the collision is
//! stepped around, so the wider form buys nothing here.
//!
//! The second is reading: `@"align"` in a field position has the shape
//! of a builtin call, and a generated struct of them is harder to skim
//! than one of `align_`, `try_`, `error_`. The IDL spelling is not lost
//! — it is written into the doc comment above every name the escape
//! moved.
//!
//! ## The statement layer
//!
//! The tree began as types only; the codec walk added [`Decl::Func`]
//! and [`Stmt`], exactly as the D tree's documentation describes for
//! its own statement layer: only the forms the shared codec walk
//! emits. Zig's richer set — `switch`, `for`, `defer`, `errdefer`,
//! payload captures — is deliberately not here, because a form no walk
//! builds is a renderer arm no test reaches.
//!
//! Two statements are Zig's own and have no D counterpart:
//!
//! - [`Stmt::Discard`] — `_ = name;`. Zig makes an unused function
//!   parameter and an unused local a compile error, and the discard is
//!   the language's way of saying the non-use is meant.
//! - [`Stmt::CountUp`] — a counted loop. Zig has no C-style `for`, so
//!   the renderer writes the idiomatic
//!   `{ var i: u32 = 0; while (i < n) : (i += 1) { … } }`, with the
//!   outer block scoping the counter so two sequential loops can share
//!   a name without shadowing.
//!
//! [`Stmt::Var`] carries `mutable`, because Zig refuses a `var` that is
//! never mutated ("local variable is never mutated") and a `const` that
//! is; the emitter decides per body, by looking at what the body
//! assigns.
//!
//! ## What is (still) not here
//!
//! No `comptime` blocks other than [`AnalyseAll`], no error set
//! declarations, no generics, no `switch`, no captures. This tree
//! carries the types an IDL file becomes and the statement forms the
//! shared codec walk emits, and nothing else.

use crate::common::{BodyText, Doc, Ident, Literal};
use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// A Zig source file.
///
/// One file per IDL file. Zig's unit of scope is the file — a file *is*
/// a struct (Zig language reference, "struct") — and it does not nest,
/// so nested IDL modules cannot nest here either. They flatten onto the
/// declared names, which is what `zerodds_lang_rules::adapter::zig` does
/// and what the C and Go columns do for the same reason.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct File {
    /// The container documentation, written as `//!` above everything.
    pub doc: Doc,
    /// Declarations, in written order.
    pub decls: Vec<Decl>,
}

/// One top-level declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decl {
    /// `pub const Name = value;` — a type or a constant, one form.
    Const(ConstDecl),
    /// A free function with a body — what a codec routine is. The
    /// types column emitted none of these; the statement layer exists
    /// for exactly this declaration.
    Func(FuncDecl),
    /// A comment standing on its own.
    Comment(Doc),
    /// `@verbatim` text, copied into the output unchanged — OMG IDL 4.2
    /// §8.3.5.1 asks for the text "verbatim", so raw lines and never a
    /// comment.
    Verbatim(String),
    /// The `test` block that forces every declaration to be analysed.
    AnalyseAll(AnalyseAll),
}

/// `pub fn name(params) Returns { body }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuncDecl {
    /// Documentation.
    pub doc: Doc,
    /// Whether an importer of the file can name it.
    pub is_pub: bool,
    /// The name declared.
    pub name: Ident,
    /// Parameters, in order. Unlike a function *type*'s parameters
    /// these always carry a name.
    pub params: Vec<FnParam>,
    /// The result type, written out — `void` where there is nothing to
    /// answer, because a Zig function states its result always.
    pub returns: Type,
    /// The body.
    pub body: Vec<Stmt>,
}

/// One named parameter of a function declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FnParam {
    /// The name.
    pub name: Ident,
    /// The type.
    pub type_: Type,
}

/// A statement, as a codec body writes one.
///
/// Only the forms the shared codec walk emits, plus the two Zig's own
/// compile errors force ([`Stmt::Discard`], and the counter block
/// [`Stmt::CountUp`] renders). The module documentation carries why
/// nothing richer is here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    /// `var name: T = value;` or `const name: T = value;`; a `None`
    /// value renders `undefined`, which is only ever written for a
    /// local the body assigns before it reads.
    Var {
        /// The name declared.
        name: Ident,
        /// The declared type; absent lets the initialiser carry it.
        type_: Option<Type>,
        /// The initialiser; absent renders `undefined`.
        value: Option<Expr>,
        /// `var` against `const`. Zig refuses each where the other is
        /// meant, so the emitter decides from what the body assigns.
        mutable: bool,
    },
    /// `place = value;`.
    Assign {
        /// The place written.
        place: Expr,
        /// The value.
        value: Expr,
    },
    /// An expression evaluated for its effect. Must be `void`-typed in
    /// Zig; a kept value goes through [`Stmt::Assign`] and a dropped
    /// one through the `_ =` of [`Stmt::Discard`] or an assign to `_`.
    Expr(Expr),
    /// `_ = value;` — a value read and deliberately not kept.
    DiscardExpr(Expr),
    /// `_ = name;` — a name deliberately not read. Zig's spelling for
    /// an unused parameter or local, which would otherwise refuse to
    /// compile.
    Discard(Ident),
    /// `return;`, with or without a value.
    Return(Option<Expr>),
    /// `if (condition) { .. } else { .. }`.
    If {
        /// The condition.
        condition: Expr,
        /// The then-branch.
        then: Vec<Stmt>,
        /// The else-branch, absent where there is none.
        otherwise: Option<Vec<Stmt>>,
    },
    /// A counted loop from zero to `bound`, exclusive, rendered as
    /// `{ var i: T = 0; while (i < bound) : (i += 1) { .. } }` — the
    /// outer block scopes the counter, so two sequential loops may use
    /// one name without Zig's shadowing error.
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
    /// `while (condition) { .. }`.
    While {
        /// The condition.
        condition: Expr,
        /// The body.
        body: Vec<Stmt>,
    },
    /// `break;`.
    Break,
    /// A comment on its own line.
    Comment(Doc),
}

/// `pub const Name: Type = value;`
///
/// The single declaration form. `type_` is the ascription and is absent
/// where the value says its own type — which is every type declaration,
/// because `pub const Pose: type = struct { … };` states what the
/// right-hand side already is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstDecl {
    /// Documentation, written as `///`.
    pub doc: Doc,
    /// Whether an importer of the file can name it.
    pub is_pub: bool,
    /// The name declared.
    pub name: Ident,
    /// The ascribed type, absent where the value carries it.
    pub type_: Option<Type>,
    /// The value.
    pub value: Expr,
    /// `@verbatim` text at the start and the end of the body, where the
    /// value is a `struct { … }`. Empty for any other value.
    pub body_text: BodyText,
}

/// The `test` block that references every declaration in the file.
///
/// Zig analyses a container's declarations only when something reaches
/// them. Measured on Zig 0.14.1: a file holding nothing but
/// `pub const X: u8 = 300;` passes `zig build-obj` with status 0, and
/// the same file under `zig test` reports
/// `type 'u8' cannot represent integer value '300'`. Without this block
/// a generated types file is checked for syntax and for nothing else.
///
/// It is a `test` block and not a `comptime` one so that it costs a
/// normal build nothing: test blocks are not compiled outside a test
/// build (Zig language reference, "Zig Test").
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AnalyseAll {
    /// The comment written above it, as `//` and not `///` — Zig
    /// refuses a documentation comment attached to a test block.
    pub doc: Doc,
}

/// One field of a struct type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// Documentation.
    pub doc: Doc,
    /// The name, as it will be written.
    pub name: Ident,
    /// The type.
    pub type_: Type,
    /// The default, for a field that has a resting value.
    ///
    /// An `@optional` member's is `null`: DDS-XTypes 1.3 §7.2.2.4.4.4.7
    /// makes absence a state of the member, and a default is how Zig
    /// says a field need not be given at construction.
    pub default: Option<Expr>,
}

/// One variant of an enumeration type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variant {
    /// Documentation.
    pub doc: Doc,
    /// The name.
    pub name: Ident,
    /// `= value`. Always written by the rules, never left to Zig's own
    /// numbering, because `@value` admits gaps.
    pub value: Option<Expr>,
}

/// One parameter of a function type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    /// The name. Zig permits one in a function *type* and ignores it;
    /// it is carried so a generated signature reads like the IDL.
    pub name: Option<Ident>,
    /// The type.
    pub type_: Type,
}

/// A Zig type, as it will be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    /// A built-in.
    Primitive(Primitive),
    /// A declared name.
    Named(Ident),
    /// `[]T` or `[]const T`.
    Slice {
        /// Whether the element is `const`.
        is_const: bool,
        /// The element type.
        element: Box<Type>,
    },
    /// `[N]T`.
    Array {
        /// The length. Part of the type in Zig, as it is in IDL.
        length: Box<Expr>,
        /// The element type.
        element: Box<Type>,
    },
    /// `?T`.
    Optional(Box<Type>),
    /// `*T` or `*const T`.
    Pointer {
        /// Whether the target is `const`.
        is_const: bool,
        /// The target type.
        target: Box<Type>,
    },
    /// `struct { … }`.
    Struct(Vec<Field>),
    /// `enum(T) { … }`.
    ///
    /// The tag type is written out. Zig would infer one from the
    /// values, and an inferred tag is not a wire decision.
    Enum {
        /// The tag type.
        tag: Box<Type>,
        /// The variants, in written order.
        variants: Vec<Variant>,
    },
    /// `fn (a: T) R`.
    Fn {
        /// Parameters, in order.
        params: Vec<Param>,
        /// The result. `void` for an operation that returns nothing.
        returns: Box<Type>,
    },
    /// `E!T` — an error union, which is what every fallible codec
    /// routine answers.
    ErrorUnion {
        /// The error set, by name.
        error_set: Box<Type>,
        /// The payload.
        payload: Box<Type>,
    },
}

impl Type {
    /// A type spelled by one name.
    #[must_use]
    pub fn named(text: &str) -> Self {
        Self::Named(Ident::new(text))
    }

    /// `[]T`.
    #[must_use]
    pub fn slice(element: Self) -> Self {
        Self::Slice {
            is_const: false,
            element: Box::new(element),
        }
    }

    /// `[]const T`.
    #[must_use]
    pub fn const_slice(element: Self) -> Self {
        Self::Slice {
            is_const: true,
            element: Box::new(element),
        }
    }

    /// `[]const u8` — the shape a `string`, a `wstring` and a
    /// `fixed<P,S>` all take.
    #[must_use]
    pub fn text() -> Self {
        Self::const_slice(Self::Primitive(Primitive::U8))
    }

    /// `[n]T`.
    #[must_use]
    pub fn array(length: i128, element: Self) -> Self {
        Self::Array {
            length: Box::new(Expr::Literal(Literal::int(length))),
            element: Box::new(element),
        }
    }

    /// `?T`.
    #[must_use]
    pub fn optional(inner: Self) -> Self {
        Self::Optional(Box::new(inner))
    }

    /// `*T`.
    #[must_use]
    pub fn pointer(target: Self) -> Self {
        Self::Pointer {
            is_const: false,
            target: Box::new(target),
        }
    }

    /// `*const T`.
    #[must_use]
    pub fn const_pointer(target: Self) -> Self {
        Self::Pointer {
            is_const: true,
            target: Box::new(target),
        }
    }

    /// `E!T`.
    #[must_use]
    pub fn error_union(error_set: Self, payload: Self) -> Self {
        Self::ErrorUnion {
            error_set: Box::new(error_set),
            payload: Box::new(payload),
        }
    }
}

/// A Zig built-in type.
///
/// `isize`, `usize` and the `c_*` family are **not** here and that is
/// the point: their width follows the target (Zig language reference,
/// "Primitive Types"), so a member declared with one would change size
/// between a build for a 64-bit host and one for a 32-bit
/// microcontroller, for a wire format that fixes it. Zig's arbitrary
/// widths — `u3`, `i17` — are absent for a different reason: no IDL
/// primitive reaches one, and a bitset's field is held in the smallest
/// standard width rather than in its exact bit count, because the
/// packing into the anchor is the codec's and the order on the wire is
/// the declaration's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Primitive {
    /// `bool`
    Bool,
    /// `i8`
    I8,
    /// `i16`
    I16,
    /// `i32`
    I32,
    /// `i64`
    I64,
    /// `u8`
    U8,
    /// `u16`
    U16,
    /// `u32`
    U32,
    /// `u64`
    U64,
    /// `f32`
    F32,
    /// `f64`
    F64,
    /// `f128` — a real binary128, which an IDL `long double` needs.
    F128,
    /// `void`
    Void,
    /// `anyopaque`, for a `native` declaration's target.
    AnyOpaque,
}

impl Primitive {
    /// Every built-in the tree can hold.
    pub const ALL: [Self; 14] = [
        Self::Bool,
        Self::I8,
        Self::I16,
        Self::I32,
        Self::I64,
        Self::U8,
        Self::U16,
        Self::U32,
        Self::U64,
        Self::F32,
        Self::F64,
        Self::F128,
        Self::Void,
        Self::AnyOpaque,
    ];

    /// How it is written.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
            Self::F32 => "f32",
            Self::F64 => "f64",
            Self::F128 => "f128",
            Self::Void => "void",
            Self::AnyOpaque => "anyopaque",
        }
    }
}

/// An expression.
///
/// The constant forms the type column always had — [`Expr::Binary`]
/// because a bitmask constant is `1 << 4`, [`Expr::Type`] because a Zig
/// type is a value — plus the call, access and `try` forms the codec
/// walk's statement layer needs. A builtin call is an ordinary
/// [`Expr::Call`] whose callee is the builtin's name, `@as` included:
/// its first argument is then an [`Expr::Type`].
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
    /// `(a)` — parentheses written on purpose.
    ///
    /// The tree carries no operator precedence, so a builder that needs
    /// a bracket says so.
    Paren(Box<Expr>),
    /// A type used as a value.
    Type(Box<Type>),
    /// `f(a, b)` — a call. The callee is an expression because a
    /// method call is a call on a dotted access, and a builtin call is
    /// one on a `@`-name.
    Call {
        /// What is called.
        func: Box<Expr>,
        /// The arguments, in order.
        args: Vec<Expr>,
    },
    /// `value.name` — a field access, a method name, or a declaration
    /// reached through its container.
    Dot {
        /// The value accessed.
        value: Box<Expr>,
        /// The name accessed.
        name: Ident,
    },
    /// `value[index]`.
    Index {
        /// The value indexed.
        value: Box<Expr>,
        /// The index.
        index: Box<Expr>,
    },
    /// `op value` — `!`, `-`, `~`, `&`, written as given with no space.
    Unary {
        /// The operator, as written.
        op: &'static str,
        /// The operand.
        value: Box<Expr>,
    },
    /// `try value` — the error propagated to the caller.
    Try(Box<Expr>),
    /// `.{ .a = x, .b = y }` — an anonymous struct literal, typed by
    /// its result location. Anonymous on purpose: every position the
    /// emitters write one into states the type — a declaration, a
    /// return, a field — and the named form would repeat it.
    StructLit(Vec<(Ident, Expr)>),
    /// `.{ a, b, c }` — an anonymous list literal, which is how an
    /// array value is written; empty and behind `&` it is the empty
    /// slice of whatever the result location wants.
    ListLit(Vec<Expr>),
}

impl Expr {
    /// An integer literal.
    #[must_use]
    pub fn int(value: i128) -> Self {
        Self::Literal(Literal::int(value))
    }

    /// A type as the right-hand side of a declaration.
    #[must_use]
    pub fn type_(type_: Type) -> Self {
        Self::Type(Box::new(type_))
    }

    /// `f(args)` on a bare name — a builtin's `@`-name included.
    #[must_use]
    pub fn call(callee: &str, args: Vec<Self>) -> Self {
        Self::Call {
            func: Box::new(Self::Name(Ident::new(callee))),
            args,
        }
    }

    /// `value.name`.
    #[must_use]
    pub fn dot(value: Self, name: &str) -> Self {
        Self::Dot {
            value: Box::new(value),
            name: Ident::new(name),
        }
    }

    /// `try value`.
    #[must_use]
    pub fn try_(value: Self) -> Self {
        Self::Try(Box::new(value))
    }

    /// `@as(T, value)` — the one cast spelling with an explicit result
    /// type, which the conversion builtins want around them.
    #[must_use]
    pub fn as_(type_: Type, value: Self) -> Self {
        Self::call("@as", alloc::vec![Self::type_(type_), value])
    }
}

/// The name an IDL identifier takes as a **field** of a struct or a
/// variant of an enumeration.
///
/// A trailing underscore where the identifier is a Zig keyword, and
/// nowhere else.
///
/// Measured on Zig 0.14.1: `struct { type: u8, u8: i32, bool: u8 }`
/// compiles and `struct { align: u8 }` does not
/// (`expected type expression, found 'align'`). A field name lives in
/// the container's field namespace, which the primitive types are not
/// in; only the keywords are excluded, because the grammar cannot get
/// past one. Renaming `type` here as well would move a member's spelling
/// for a collision the language does not have.
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
/// A trailing underscore for a Zig keyword, as [`escape_field`] does,
/// and additionally for every primitive name.
///
/// Measured on Zig 0.14.1: `pub const u8 = i32;` is
/// `error: name shadows primitive 'u8'`, and so is `pub const type`,
/// `pub const bool` and `pub const u7`. Unlike Go — where shadowing a
/// predeclared identifier is legal and merely unwise — Zig refuses it
/// outright, so this is not a precaution but the condition for the file
/// to compile at all.
///
/// The arbitrary-width integers are matched by shape and not by a list:
/// Zig admits `u0` through `u65535` and the same range of `i`, and a
/// list of 131 072 names would be the wrong way to say
/// [`is_primitive_name`]'s one rule.
#[must_use]
pub fn escape_decl(name: &str) -> String {
    if KEYWORDS.contains(&name) || is_primitive_name(name) {
        let mut out = name.to_string();
        out.push('_');
        return out;
    }
    name.to_string()
}

/// Whether `name` is one of Zig's primitive names.
///
/// The named ones from the language reference's "Primitive Types" and
/// "Primitive Values", plus the arbitrary-width integers `u<N>` and
/// `i<N>`. `u007` is **not** one: Zig's grammar takes the digits after
/// the letter as a decimal bit count with no leading zero, and treating
/// it as one would escape a name Zig would have accepted.
#[must_use]
pub fn is_primitive_name(name: &str) -> bool {
    if PRIMITIVE_NAMES.contains(&name) {
        return true;
    }
    let Some(digits) = name.strip_prefix('u').or_else(|| name.strip_prefix('i')) else {
        return false;
    };
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return false;
    }
    // `u0` is a type; `u01` is not, because the width is written
    // without a leading zero.
    digits == "0" || !digits.starts_with('0')
}

/// Zig's keywords. Zig 0.14 language reference, "Keyword Reference".
///
/// A keyword cannot be an identifier anywhere without `@"…"`, which
/// this column does not use — the module documentation says why.
pub const KEYWORDS: &[&str] = &[
    "addrspace",
    "align",
    "allowzero",
    "and",
    "anyframe",
    "anytype",
    "asm",
    "async",
    "await",
    "break",
    "callconv",
    "catch",
    "comptime",
    "const",
    "continue",
    "defer",
    "else",
    "enum",
    "errdefer",
    "error",
    "export",
    "extern",
    "fn",
    "for",
    "if",
    "inline",
    "linksection",
    "noalias",
    "noinline",
    "nosuspend",
    "opaque",
    "or",
    "orelse",
    "packed",
    "pub",
    "resume",
    "return",
    "struct",
    "suspend",
    "switch",
    "test",
    "threadlocal",
    "try",
    "union",
    "unreachable",
    "usingnamespace",
    "var",
    "volatile",
    "while",
];

/// Zig's named primitive types and primitive values.
///
/// Not keywords — the grammar parses them as identifiers — but a
/// declaration may not take one of these names either, which is the
/// difference from Go, where shadowing a predeclared identifier
/// compiles.
///
/// `isize`, `usize` and the `c_*` family are listed here although
/// [`Primitive`] refuses to *write* them: a name has to be escaped
/// whether or not this column would ever emit the type it collides with.
pub const PRIMITIVE_NAMES: &[&str] = &[
    // Primitive types.
    "anyerror",
    "anyframe",
    "anyopaque",
    "bool",
    "c_char",
    "c_int",
    "c_long",
    "c_longdouble",
    "c_longlong",
    "c_short",
    "c_uint",
    "c_ulong",
    "c_ulonglong",
    "c_ushort",
    "comptime_float",
    "comptime_int",
    "f16",
    "f32",
    "f64",
    "f80",
    "f128",
    "isize",
    "noreturn",
    "type",
    "usize",
    "void",
    // Primitive values.
    "false",
    "null",
    "true",
    "undefined",
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
    fn no_primitive_has_a_target_dependent_width() {
        // `isize`, `usize` and the `c_*` family follow the target (Zig
        // language reference, "Primitive Types"). A member declared with
        // one would change size between a 64-bit host and a 32-bit
        // microcontroller, for a wire format that fixes every width.
        for primitive in Primitive::ALL {
            let text = primitive.as_str();
            assert!(
                !matches!(text, "isize" | "usize") && !text.starts_with("c_"),
                "{text} has no fixed width"
            );
        }
    }

    #[test]
    fn the_tree_can_write_a_binary128() {
        // The one built-in that separates this column from Go's and
        // Python's, which hold an IDL `long double` as decimal text
        // because their languages have nothing 113 bits wide.
        assert!(Primitive::ALL.contains(&Primitive::F128));
        assert_eq!(Primitive::F128.as_str(), "f128");
    }

    #[test]
    fn a_keyword_escapes_in_both_positions() {
        for name in ["align", "error", "test", "try", "fn", "comptime"] {
            assert_eq!(escape_field(name), alloc::format!("{name}_"));
            assert_eq!(escape_decl(name), alloc::format!("{name}_"));
        }
    }

    #[test]
    fn a_primitive_name_escapes_as_a_declaration_and_not_as_a_field() {
        // Measured on Zig 0.14.1: `struct { type: u8, u8: i32 }`
        // compiles; `pub const u8 = i32;` is
        // `error: name shadows primitive 'u8'`. A field lives in the
        // field namespace, which the primitives are not in.
        for name in ["type", "u8", "bool", "f128", "null", "usize"] {
            assert_eq!(escape_field(name), name);
            assert_eq!(escape_decl(name), alloc::format!("{name}_"));
        }
    }

    #[test]
    fn an_arbitrary_width_integer_is_a_primitive_name_too() {
        // Zig admits `u0` through `u65535` and the same range of `i`.
        for name in ["u0", "u1", "u7", "i17", "u65535"] {
            assert!(is_primitive_name(name), "{name} is a Zig type");
        }
        // Not written as a width, so not a type: the digits carry no
        // leading zero, and escaping these would move a name Zig
        // accepts.
        for name in ["u01", "uX", "u", "i", "u1a", "unit", "index"] {
            assert!(!is_primitive_name(name), "{name} is not a Zig type");
        }
    }

    #[test]
    fn a_plain_name_is_left_exactly_as_the_idl_wrote_it() {
        // No case is changed anywhere: Zig has no export-by-case rule,
        // so renaming a member would be a rename with no cause.
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
        // `long grid[2][3]` is two rows of three, which in Zig is
        // `[2][3]i32`. Reversing it silently transposes the data.
        let type_ = Type::array(2, Type::array(3, Type::Primitive(Primitive::I32)));
        let Type::Array { length, element } = &type_ else {
            panic!("not an array");
        };
        assert_eq!(**length, Expr::int(2));
        let Type::Array { length, .. } = &**element else {
            panic!("the element is not an array");
        };
        assert_eq!(**length, Expr::int(3));
    }

    #[test]
    fn text_is_a_const_slice_of_octets() {
        // `string`, `wstring` and `fixed<P,S>` all take this shape; the
        // adapter's documentation says why.
        let Type::Slice { is_const, element } = Type::text() else {
            panic!("not a slice");
        };
        assert!(is_const, "a string literal does not coerce to `[]u8`");
        assert_eq!(*element, Type::Primitive(Primitive::U8));
    }

    #[test]
    fn an_optional_field_can_carry_its_absence_as_a_default() {
        let field = Field {
            doc: Doc::none(),
            name: Ident::new("b"),
            type_: Type::optional(Type::Primitive(Primitive::U8)),
            default: Some(Expr::Literal(Literal::Null)),
        };
        assert_eq!(field.default, Some(Expr::Literal(Literal::Null)));
    }

    #[test]
    fn a_type_is_a_value_so_one_declaration_form_carries_both() {
        let as_type = ConstDecl {
            body_text: crate::common::BodyText::none(),
            doc: Doc::none(),
            is_pub: true,
            name: Ident::new("Pose"),
            type_: None,
            value: Expr::type_(Type::Struct(Vec::new())),
        };
        let as_constant = ConstDecl {
            body_text: crate::common::BodyText::none(),
            doc: Doc::none(),
            is_pub: true,
            name: Ident::new("LIMIT"),
            type_: Some(Type::Primitive(Primitive::U8)),
            value: Expr::int(8),
        };
        assert!(matches!(as_type.value, Expr::Type(_)));
        assert!(as_constant.type_.is_some());
    }
}
