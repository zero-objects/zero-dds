// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The D concrete syntax tree. Spec §1.
//!
//! D's declaration syntax is C's with the declarator straightened out:
//! a declaration is `BasicType TypeSuffixes Identifier`, and every
//! suffix binds to the type rather than to the name. So, as in
//! [`crate::go`] and unlike [`crate::c`], the tree carries no declarator
//! and the renderer never splits a type around a name. What it does
//! carry, and Go's does not, is the **direction the suffixes read**.
//!
//! ## The one thing this tree gets right that a C-shaped one would not
//!
//! In D a type suffix is appended and read right to left: `int[3][2]` is
//! **two arrays of three ints** (D language specification, "Arrays",
//! "Rectangular Arrays"), which is the opposite of C, where
//! `int a[3][2]` is three arrays of two. IDL `long grid[2][3]` is two
//! rows of three (IDL 4.2 §7.4.1.4.4.4), so the D declaration is
//! `int[3][2] grid`.
//!
//! [`Type::Array`] therefore holds the **outermost** dimension outermost,
//! exactly as [`crate::go::Type::Array`] does — the two trees are built
//! identically and only `zerodds_lang_render::d` differs, writing the
//! length after the element instead of before it. Building the tree the
//! other way round would transpose every multi-dimensional array with
//! nothing in the output to say so.
//!
//! ## What is not here
//!
//! No `real`, no `cent`, no `char`/`wchar`/`dchar`, no `class`, no
//! function bodies, no templates but the one [`Type::Nullable`] names,
//! no `version` blocks, no `mixin`. [`Primitive`] carries why each
//! absence is deliberate.
//!
//! Every form has a renderer test, so no arm of
//! `zerodds_lang_render::d` is code that only claims to work.

use crate::common::{Doc, Ident, Literal, Path};
use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// A D source file.
///
/// One file, one module. D's module declaration names the whole file
/// (D language specification, "Modules": "There is a one-to-one
/// correspondence between source files and modules"), so nested IDL
/// modules cannot each become a D module here — they flatten onto the
/// declared names, which is what `zerodds_lang_rules::adapter::d` does
/// and what the C and Go columns do for their own reasons.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct File {
    /// The leading comment, written above the module declaration.
    pub doc: Doc,
    /// The module name: `module a.b;`.
    ///
    /// Required. A D file without one is given a module name from its
    /// file name, and a generated file whose module name depends on
    /// where it was written is one an importer cannot name.
    pub module: Path,
    /// `import` lines, in written order.
    pub imports: Vec<Import>,
    /// Declarations, in written order.
    pub decls: Vec<Decl>,
}

/// A file declaring the module a generated file always declares.
///
/// Written out rather than derived: [`Path`] has no `Default`, and an
/// empty module name is not a D file.
impl Default for File {
    fn default() -> Self {
        Self {
            doc: Doc::none(),
            module: Path::new(&["zdtypes"]),
            imports: Vec::new(),
            decls: Vec::new(),
        }
    }
}

/// One `import` line.
///
/// `symbols` is D's selective import — `import std.typecons : Nullable;`
/// binds one name instead of the module's whole namespace (D language
/// specification, "Modules", "Selective Imports"). A generated file uses
/// the selective form so that a declaration it emits cannot be shadowed
/// by a name Phobos happens to export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Import {
    /// The module path, dot-separated when written.
    pub path: Path,
    /// The names bound, empty for a whole-module import.
    pub symbols: Vec<Ident>,
}

impl Import {
    /// `import <path> : <symbol>;`.
    #[must_use]
    pub fn selective(path: &[&str], symbol: &str) -> Self {
        Self {
            path: Path::new(path),
            symbols: alloc::vec![Ident::new(symbol)],
        }
    }

    /// `import <path>;` — the whole module, reached by its qualified
    /// name.
    #[must_use]
    pub fn module(path: &[&str]) -> Self {
        Self {
            path: Path::new(path),
            symbols: Vec::new(),
        }
    }
}

/// One top-level declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decl {
    /// `struct Name { ... }`.
    ///
    /// A `struct` and never a `class`. A D `class` is a reference type
    /// with a vtable and a monitor field, allocated on the GC heap
    /// (D language specification, "Classes"); the wire form of an IDL
    /// aggregate is a value, and a column that made it a reference would
    /// put an indirection and a null state between a member and the
    /// octets it is written from.
    Struct(StructDecl),
    /// `enum Name : Base { ... }`.
    Enum(EnumDecl),
    /// `alias Name = T;`.
    Alias(AliasDecl),
    /// `enum T Name = value;` — a manifest constant.
    Const(ConstDecl),
    /// `interface Name { ... }`.
    Interface(InterfaceDecl),
    /// A free function with a body — what a codec routine is. The
    /// types column emitted none of these; the statement layer below
    /// exists for exactly this declaration.
    Func(FuncDecl),
    /// A comment standing on its own.
    Comment(Doc),
    /// `@verbatim` text, copied into the output unchanged — OMG IDL 4.2
    /// §8.3.5.1 asks for the text "verbatim", so raw lines and never a
    /// comment.
    Verbatim(String),
}

/// `ReturnType name(params) { body }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuncDecl {
    /// Documentation.
    pub doc: Doc,
    /// The name declared.
    pub name: Ident,
    /// Parameters, in order.
    pub params: Vec<Param>,
    /// The result type, absent for `void`.
    pub returns: Option<Type>,
    /// The body.
    pub body: Vec<Stmt>,
}

/// A statement, as a codec body writes one.
///
/// Only the forms the shared codec walk emits. D's own richer set —
/// `switch`, `foreach`, `scope`, contracts — is deliberately not here:
/// a form no walk builds is a renderer arm no test reaches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    /// `T name = value;`, `T name;`, or `auto name = value;` where the
    /// type is the initialiser's.
    Var {
        /// The name declared.
        name: Ident,
        /// The declared type; absent asks for `auto` and then a value
        /// is required.
        type_: Option<Type>,
        /// The initialiser.
        value: Option<Expr>,
    },
    /// `place = value;`.
    Assign {
        /// The place written.
        place: Expr,
        /// The value.
        value: Expr,
    },
    /// An expression evaluated for its effect.
    Expr(Expr),
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
    /// `for (T counter = 0; counter < bound; counter++) { .. }`.
    ///
    /// The counter's type is the caller's, because the bounds it is
    /// compared against carry theirs — a `uint` count off the wire
    /// against an `int` counter is a signedness the compiler flags.
    For {
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

/// `struct Name { ... }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructDecl {
    /// Documentation.
    pub doc: Doc,
    /// The name declared.
    pub name: Ident,
    /// The members, in written order.
    pub members: Vec<Member>,
}

/// One member of a struct.
///
/// Two forms, because a D struct can hold an anonymous `union` as a
/// member and an IDL union needs exactly that: a discriminator beside
/// overlaid branches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Member {
    /// An ordinary field.
    Field(Field),
    /// `union { ... }` — an anonymous union member.
    ///
    /// Its fields are named directly on the enclosing struct
    /// (D language specification, "Structs, Unions"), so `payload.idle`
    /// reaches a branch without an intermediate name that the IDL never
    /// gave.
    Union(Vec<Field>),
    /// `@verbatim` text inside the struct body, copied unchanged.
    Verbatim(String),
}

/// One field of a struct or of an anonymous union.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// Documentation.
    pub doc: Doc,
    /// User-defined attributes, in written order.
    pub udas: Vec<Uda>,
    /// The name, as it will be written.
    pub name: Ident,
    /// The type.
    pub type_: Type,
}

/// One user-defined attribute: `@("idl:x")`.
///
/// D's own compile-time readable annotation, recovered with
/// `__traits(getAttributes, S.x)` (D language specification,
/// "Attributes", "User-Defined Attributes"). This is where the IDL
/// spelling of a renamed member goes — the counterpart of the Go
/// column's struct tag, and of the macro the C column has to write
/// beside the declaration because C has neither.
///
/// It differs from a Go tag in when it can be read: a UDA is available
/// at compile time and a struct tag at run time. A generated codec that
/// walks `.tupleof` in a `static foreach` reads it at the point where it
/// needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Uda {
    /// The single string the attribute carries.
    pub text: String,
}

impl Uda {
    /// `@("<key>:<value>")`.
    #[must_use]
    pub fn keyed(key: &str, value: &str) -> Self {
        let mut text = key.to_string();
        text.push(':');
        text.push_str(value);
        Self { text }
    }
}

/// `enum Name : Base { ... }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumDecl {
    /// Documentation.
    pub doc: Doc,
    /// The name declared.
    pub name: Ident,
    /// The base type, written out.
    ///
    /// D infers `int` when it is absent, and an IDL enumeration's holder
    /// is a decision the source made rather than one the target should
    /// guess, so it is stated.
    pub base: Option<Type>,
    /// The members, in written order. Never empty: D rejects an enum
    /// body with no members.
    pub members: Vec<EnumMember>,
}

/// One member of an enumeration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumMember {
    /// Documentation.
    pub doc: Doc,
    /// The name.
    pub name: Ident,
    /// The value, always written out.
    ///
    /// D numbers an unvalued member one above its predecessor, and
    /// `@value` admits gaps (DDS-XTypes 1.3 §7.4.1.1.3.1), so a member
    /// left unvalued here would renumber the wire.
    pub value: Expr,
}

/// `alias Name = T;`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AliasDecl {
    /// Documentation.
    pub doc: Doc,
    /// The name declared.
    pub name: Ident,
    /// What it names.
    pub type_: Type,
}

/// `enum T Name = value;` — a manifest constant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstDecl {
    /// Documentation.
    pub doc: Doc,
    /// The name declared.
    pub name: Ident,
    /// The declared type, absent where D is to infer it.
    pub type_: Option<Type>,
    /// The value.
    pub value: Expr,
}

/// `interface Name { ... }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceDecl {
    /// Documentation.
    pub doc: Doc,
    /// The name declared.
    pub name: Ident,
    /// The methods, in written order.
    pub methods: Vec<Method>,
}

/// One method of an interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Method {
    /// Documentation.
    pub doc: Doc,
    /// The name.
    pub name: Ident,
    /// The return type, absent for an operation returning `void`.
    pub returns: Option<Type>,
    /// Parameters, in order.
    pub params: Vec<Param>,
}

/// One parameter of a method.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    /// The name.
    pub name: Ident,
    /// The type.
    pub type_: Type,
}

/// A D type, as it will be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    /// A built-in.
    Primitive(Primitive),
    /// A declared name.
    Named(Ident),
    /// `T[]` — a dynamic array, D's slice.
    Slice(Box<Type>),
    /// `T[N]` — a static array.
    ///
    /// The **outermost** dimension is outermost here, and the renderer
    /// writes it last: `Array { 2, Array { 3, int } }` is `int[3][2]`,
    /// which D reads as two arrays of three.
    Array {
        /// The length. Part of the type in D, as it is in IDL.
        length: Box<Expr>,
        /// The element type.
        element: Box<Type>,
    },
    /// `T*`.
    Pointer(Box<Type>),
    /// `Nullable!(T)` from `std.typecons`.
    ///
    /// The template argument is always parenthesised. `Nullable!T[]`
    /// parses as `(Nullable!T)[]`, because D's single-token template
    /// argument binds to the token and not to the type suffix after it
    /// (D language specification, "Templates",
    /// "TemplateSingleArgument"), and a generated `Nullable!int[]` where
    /// `Nullable!(int[])` was meant would compile and hold the wrong
    /// thing.
    Nullable(Box<Type>),
}

impl Type {
    /// A type spelled by one name.
    #[must_use]
    pub fn named(text: &str) -> Self {
        Self::Named(Ident::new(text))
    }

    /// `T[]`.
    #[must_use]
    pub fn slice(element: Self) -> Self {
        Self::Slice(Box::new(element))
    }

    /// `T*`.
    #[must_use]
    pub fn pointer(target: Self) -> Self {
        Self::Pointer(Box::new(target))
    }

    /// `Nullable!(T)`.
    #[must_use]
    pub fn nullable(target: Self) -> Self {
        Self::Nullable(Box::new(target))
    }

    /// `T[n]`, with `n` the outermost dimension.
    #[must_use]
    pub fn array(length: i128, element: Self) -> Self {
        Self::Array {
            length: Box::new(Expr::Literal(Literal::int(length))),
            element: Box::new(element),
        }
    }
}

/// A D basic type.
///
/// Four D basic types are deliberately absent, and each absence is a
/// decision this column had to make:
///
/// - **`real`** is 80-bit x87 extended on x86 and x86-64, and the same
///   as `double` on AArch64 and on PowerPC — its width and its
///   significand are the target's, which is the same reason the C column
///   refuses the compiler's `long double`. An IDL `long double` is held
///   as a [`Primitive::String`] carrying the decimal instead.
/// - **`cent`/`ucent`** are reserved and not implemented by any D
///   compiler, so a declaration using one does not compile.
/// - **`char`, `wchar`, `dchar`** are UTF code units in D — a `char` is
///   one octet *of a UTF-8 sequence* and a `wchar` one UTF-16 code unit
///   (D language specification, "Types") — and neither is an IDL `char`
///   or `wchar`, which are an octet and a 16-bit code unit with no
///   encoding attached. Assigning an arbitrary octet to a D `char` is
///   accepted and then produces an invalid UTF-8 sequence the moment
///   anything in Phobos looks at it. The integers `ubyte` and `ushort`
///   are used instead.
/// - **`ifloat`, `idouble`, `ireal`, `cfloat`, `cdouble`, `creal`** are
///   the imaginary and complex types, deprecated in D and reached by no
///   IDL construct.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Primitive {
    /// `void` — a return type and the target of an opaque pointer.
    Void,
    /// `bool`
    Bool,
    /// `byte` — **8 bits, signed**.
    Byte,
    /// `ubyte` — 8 bits, unsigned.
    UByte,
    /// `short` — 16 bits, signed.
    Short,
    /// `ushort` — 16 bits, unsigned.
    UShort,
    /// `int` — **32 bits**, which is what an IDL `long` is.
    Int,
    /// `uint` — 32 bits, unsigned.
    UInt,
    /// `long` — **64 bits**, which is what an IDL `long long` is and
    /// what an IDL `long` is **not**.
    Long,
    /// `ulong` — 64 bits, unsigned.
    ULong,
    /// `float` — binary32.
    Float,
    /// `double` — binary64.
    Double,
    /// `string` — `immutable(char)[]`, UTF-8.
    String,
}

impl Primitive {
    /// Every built-in the tree can hold.
    pub const ALL: [Self; 13] = [
        Self::Void,
        Self::Bool,
        Self::Byte,
        Self::UByte,
        Self::Short,
        Self::UShort,
        Self::Int,
        Self::UInt,
        Self::Long,
        Self::ULong,
        Self::Float,
        Self::Double,
        Self::String,
    ];

    /// How it is written.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Void => "void",
            Self::Bool => "bool",
            Self::Byte => "byte",
            Self::UByte => "ubyte",
            Self::Short => "short",
            Self::UShort => "ushort",
            Self::Int => "int",
            Self::UInt => "uint",
            Self::Long => "long",
            Self::ULong => "ulong",
            Self::Float => "float",
            Self::Double => "double",
            Self::String => "string",
        }
    }

    /// How many bits it occupies, where the width is fixed.
    ///
    /// `None` for `void` and for `string`, which have no width of their
    /// own. Every other member has one and it is the same on every
    /// target D compiles for (D language specification, "Types",
    /// "Basic Data Types") — which is the property `real` does not have
    /// and the reason it is not in this enumeration.
    #[must_use]
    pub const fn bits(self) -> Option<u16> {
        Some(match self {
            Self::Void | Self::String => return None,
            Self::Bool | Self::Byte | Self::UByte => 8,
            Self::Short | Self::UShort => 16,
            Self::Int | Self::UInt | Self::Float => 32,
            Self::Long | Self::ULong | Self::Double => 64,
        })
    }
}

/// An expression.
///
/// Only the forms a manifest constant, an enum member and an array
/// length take. D requires a static array's length to be a compile-time
/// constant, which is why [`Expr::Binary`] is here at all: a bitmask
/// constant is `1 << 4`.
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
    /// `cast(T)v`.
    Cast {
        /// The target type.
        type_: Box<Type>,
        /// The value.
        value: Box<Expr>,
    },
    /// `(a)` — parentheses written on purpose.
    ///
    /// The tree carries no operator precedence, so a builder that needs
    /// a bracket says so.
    Paren(Box<Expr>),
    /// `f(a, b)` — a call. The callee is an expression because a method
    /// call is a call on a dotted access.
    Call {
        /// What is called.
        func: Box<Expr>,
        /// The arguments, in order.
        args: Vec<Expr>,
    },
    /// `value.name` — a member access, a method name, or a property.
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
    /// `value[0 .. high]` — a slice keeping the first elements.
    SliceTo {
        /// The value sliced.
        value: Box<Expr>,
        /// The exclusive end.
        high: Box<Expr>,
    },
    /// `op value` — `!`, `-`, `~` (D's bitwise complement), `*` and
    /// `&`, written as given with no space.
    Unary {
        /// The operator, as written.
        op: &'static str,
        /// The operand.
        value: Box<Expr>,
    },
    /// `Name(field: value, ...)` — a struct literal with named
    /// arguments, which is what keeps two same-typed members from
    /// transposing silently.
    StructLit {
        /// The struct's name.
        name: Ident,
        /// The fields, in written order; a `None` key is positional.
        fields: Vec<(Option<Ident>, Expr)>,
    },
    /// `[a, b, c]` — an array literal.
    ArrayLit(Vec<Expr>),
    /// `new T[](length)` — a freshly allocated slice of `length`
    /// elements, each at its type's `.init`.
    NewSlice {
        /// The element type.
        element: Box<Type>,
        /// The element count.
        length: Box<Expr>,
    },
}

impl Expr {
    /// An integer literal.
    #[must_use]
    pub fn int(value: i128) -> Self {
        Self::Literal(Literal::int(value))
    }

    /// An integer literal written with a suffix — `1UL`.
    ///
    /// D types an unsuffixed integer literal as `int`, and a shift count
    /// outside `0 .. 31` on an `int` is rejected at compile time
    /// (D language specification, "Expressions", "Shift Expressions").
    /// A bitmask bit at position 31 or above therefore has to shift a
    /// `ulong`, which is what the suffix says.
    #[must_use]
    pub fn suffixed_int(value: i128, suffix: &str) -> Self {
        let mut text = alloc::format!("{value}");
        text.push_str(suffix);
        Self::Literal(Literal::Int(text))
    }
}

/// The name an IDL identifier takes in D.
///
/// A trailing underscore where the identifier is a D keyword or one of
/// the module-scope names a generated file relies on. D has no
/// raw-identifier syntax — nothing like Rust's `r#type` or C#'s `@class`
/// — so the trailing underscore is the whole mechanism, as it is in the
/// C, Go and Python columns.
///
/// The IDL name is not lost when this fires: it is written into the
/// field's [`Uda`] and into the comment above every other kind of
/// declaration.
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
/// Three groups, and only the first is a keyword:
///
/// 1. **D's keywords**, from the D language specification, "Lexical",
///    "Keywords". `body` is included: it was a keyword until D 2.101 and
///    is still one for every compiler older than that.
/// 2. **The type names `object.d` declares** — `string`, `wstring`,
///    `dstring`, `size_t`, `ptrdiff_t`, `Object`. These are not
///    keywords; D lets a module-scope declaration shadow one, and the
///    file compiles until a sibling declaration wants the original. A
///    generated file wants the originals: every second field in it is a
///    `string`. This is the hazard the Go column documents for its
///    predeclared identifiers, and D has it for the same reason.
/// 3. **The names a generated file's own imports bind** — `std` and
///    `Nullable`. `import std.typecons : Nullable;` puts both at module
///    scope, and a declared type called `Nullable` would make every
///    optional member in the file mean something else.
///
/// The built-in properties every type has — `init`, `sizeof`,
/// `alignof`, `mangleof`, `stringof`, `tupleof` (D language
/// specification, "Properties") — are here too. They are reached as
/// `S.init` and `S.tupleof`, which is exactly how a generated codec
/// walks a struct, and a member with one of those names takes the name
/// over.
pub const RESERVED: &[&str] = &[
    // Keywords.
    "abstract",
    "alias",
    "align",
    "asm",
    "assert",
    "auto",
    "body",
    "bool",
    "break",
    "byte",
    "case",
    "cast",
    "catch",
    "cdouble",
    "cent",
    "cfloat",
    "char",
    "class",
    "const",
    "continue",
    "creal",
    "dchar",
    "debug",
    "default",
    "delegate",
    "delete",
    "deprecated",
    "do",
    "double",
    "else",
    "enum",
    "export",
    "extern",
    "false",
    "final",
    "finally",
    "float",
    "for",
    "foreach",
    "foreach_reverse",
    "function",
    "goto",
    "idouble",
    "if",
    "ifloat",
    "immutable",
    "import",
    "in",
    "inout",
    "int",
    "interface",
    "invariant",
    "ireal",
    "is",
    "lazy",
    "long",
    "macro",
    "mixin",
    "module",
    "new",
    "nothrow",
    "null",
    "out",
    "override",
    "package",
    "pragma",
    "private",
    "protected",
    "public",
    "pure",
    "real",
    "ref",
    "return",
    "scope",
    "shared",
    "short",
    "static",
    "struct",
    "super",
    "switch",
    "synchronized",
    "template",
    "this",
    "throw",
    "true",
    "try",
    "typeid",
    "typeof",
    "ubyte",
    "ucent",
    "uint",
    "ulong",
    "union",
    "unittest",
    "ushort",
    "version",
    "void",
    "wchar",
    "while",
    "with",
    "__FILE__",
    "__FILE_FULL_PATH__",
    "__FUNCTION__",
    "__LINE__",
    "__MODULE__",
    "__PRETTY_FUNCTION__",
    "__gshared",
    "__parameters",
    "__traits",
    "__vector",
    // What `object.d` declares, implicitly imported into every module.
    "Object",
    "TypeInfo",
    "dstring",
    "ptrdiff_t",
    "size_t",
    "string",
    "wstring",
    // What a generated file's own import binds.
    "Nullable",
    "std",
    // Built-in properties, reached as `S.init` and `S.tupleof`.
    "alignof",
    "init",
    "mangleof",
    "sizeof",
    "stringof",
    "tupleof",
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
        // `real` is 80-bit x87 extended on x86 and a plain `double` on
        // AArch64. `cent` and `ucent` are reserved and unimplemented.
        // A member declared with any of them would change size or fail
        // to compile depending on where the file is built, for a wire
        // format that fixes every width.
        for primitive in Primitive::ALL {
            assert!(
                !matches!(
                    primitive.as_str(),
                    "real" | "cent" | "ucent" | "creal" | "ireal"
                ),
                "{} has no fixed width on every target",
                primitive.as_str()
            );
        }
    }

    #[test]
    fn no_primitive_is_a_utf_code_unit() {
        // D's `char`, `wchar` and `dchar` are UTF-8, UTF-16 and UTF-32
        // code units. An IDL `char` is one octet and an IDL `wchar` a
        // 16-bit code unit with no encoding attached, so neither maps
        // onto a D character type: the integers do.
        for primitive in Primitive::ALL {
            assert!(
                !matches!(primitive.as_str(), "char" | "wchar" | "dchar"),
                "{} is a UTF code unit and not an IDL character",
                primitive.as_str()
            );
        }
    }

    #[test]
    fn a_d_int_is_thirty_two_bits_and_a_d_long_is_sixty_four() {
        // The single most likely defect in this column. An IDL `long` is
        // 32 bits (IDL 4.2 §7.4.1.4.4.2) and a D `long` is 64, so a
        // column that mapped `long` onto `long` by name would double the
        // width of every 32-bit member.
        assert_eq!(Primitive::Int.bits(), Some(32));
        assert_eq!(Primitive::Long.bits(), Some(64));
        assert_eq!(Primitive::UInt.bits(), Some(32));
        assert_eq!(Primitive::ULong.bits(), Some(64));
    }

    #[test]
    fn only_void_and_string_have_no_width() {
        for primitive in Primitive::ALL {
            let has_width = primitive.bits().is_some();
            assert_eq!(
                has_width,
                !matches!(primitive, Primitive::Void | Primitive::String),
                "{} disagrees about having a width",
                primitive.as_str()
            );
        }
    }

    #[test]
    fn a_keyword_escapes_with_a_trailing_underscore() {
        assert_eq!(escape_name("version"), "version_");
        assert_eq!(escape_name("scope"), "scope_");
        assert_eq!(escape_name("payload"), "payload");
    }

    #[test]
    fn a_type_keyword_escapes_because_a_declaration_could_not_shadow_it() {
        for name in ["int", "long", "real", "string", "union", "byte"] {
            assert_eq!(escape_name(name), alloc::format!("{name}_"));
        }
    }

    #[test]
    fn a_name_the_generated_import_binds_escapes_too() {
        // `import std.typecons : Nullable;` puts both at module scope.
        // A declared type called `Nullable` would make every optional
        // member in the file mean something else.
        assert_eq!(escape_name("Nullable"), "Nullable_");
        assert_eq!(escape_name("std"), "std_");
    }

    #[test]
    fn a_built_in_property_name_escapes() {
        // `S.init` and `S.tupleof` are how a generated codec walks a
        // struct; a member with one of those names takes the name over.
        assert_eq!(escape_name("init"), "init_");
        assert_eq!(escape_name("tupleof"), "tupleof_");
    }

    #[test]
    fn the_reserved_list_has_no_duplicates() {
        let mut seen: Vec<&str> = Vec::new();
        for name in RESERVED {
            assert!(!seen.contains(name), "{name} is listed twice");
            seen.push(name);
        }
    }

    #[test]
    fn every_primitive_spelling_is_reserved_or_deliberately_not() {
        // A generated declaration named after a type this column emits
        // would shadow the type. `void` and the rest are keywords;
        // `string` is not, and is on the list for that reason.
        for primitive in Primitive::ALL {
            assert!(
                RESERVED.contains(&primitive.as_str()),
                "{} can be shadowed",
                primitive.as_str()
            );
        }
    }

    #[test]
    fn an_array_type_nests_outermost_dimension_first() {
        // `long grid[2][3]` is two rows of three. In D that is written
        // `int[3][2]`, and the renderer produces it by writing the
        // element first — so the *tree* holds 2 outermost, exactly as
        // the Go tree does, and only the rendering direction differs.
        let type_ = Type::array(2, Type::array(3, Type::Primitive(Primitive::Int)));
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
    fn a_field_carries_its_idl_spelling_in_a_uda() {
        let field = Field {
            doc: Doc::none(),
            udas: alloc::vec![Uda::keyed("idl", "version")],
            name: Ident::new("version_"),
            type_: Type::Primitive(Primitive::Int),
        };
        assert_eq!(field.udas.len(), 1);
        assert_eq!(field.udas[0].text, "idl:version");
    }

    #[test]
    fn a_struct_can_hold_an_anonymous_union_member() {
        let decl = StructDecl {
            doc: Doc::none(),
            name: Ident::new("Payload"),
            members: alloc::vec![
                Member::Field(Field {
                    doc: Doc::none(),
                    udas: Vec::new(),
                    name: Ident::new("_d"),
                    type_: Type::Primitive(Primitive::Int),
                }),
                Member::Union(alloc::vec![Field {
                    doc: Doc::none(),
                    udas: Vec::new(),
                    name: Ident::new("idle"),
                    type_: Type::Primitive(Primitive::Int),
                }]),
            ],
        };
        assert!(matches!(decl.members[1], Member::Union(_)));
    }

    #[test]
    fn a_suffixed_literal_keeps_the_suffix() {
        assert_eq!(
            Expr::suffixed_int(1, "UL"),
            Expr::Literal(Literal::Int("1UL".into()))
        );
        assert_eq!(Expr::suffixed_int(1, ""), Expr::int(1));
    }

    #[test]
    fn a_selective_import_binds_one_name() {
        let import = Import::selective(&["std", "typecons"], "Nullable");
        assert_eq!(import.path.segments().len(), 2);
        assert_eq!(import.symbols.len(), 1);
    }
}
