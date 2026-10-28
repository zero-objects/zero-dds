// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Go concrete syntax tree. Spec §1.
//!
//! Go's declaration syntax is the reverse of C's — the name comes first
//! and the type is one expression that reads left to right — so the tree
//! carries no declarator at all: `[3][2]int32` is a [`Type::Array`]
//! holding a [`Type::Array`] holding a [`Type::Primitive`], and the
//! renderer never has to split a type around a name the way
//! `zerodds_lang_render::c` does.
//!
//! Three things the tree holds that no other column's does, and each is
//! Go's:
//!
//! 1. **A struct tag.** Go's own mechanism for putting text on a field
//!    that a program can read back at run time (`reflect.StructTag`).
//!    That is where the IDL spelling of a renamed member goes — see
//!    [`Field::tag`] — rather than in a macro beside the declaration, as
//!    `zerodds_lang_rules::adapter::c` has to do because C has nothing
//!    like it.
//! 2. **An alias declaration beside a defined type.** `type A = B` and
//!    `type A B` are different declarations in Go (spec, "Type
//!    declarations"): the first is another name for one type, the second
//!    is a new type with no methods of B's. An IDL `typedef` is the
//!    first, and a tree that could only write the second would make
//!    `Distance` and `int32` two types the compiler keeps apart.
//! 3. **No visibility field.** Go has no `public` keyword:
//!    [`crate::common::Visibility`] has no Go spelling, because whether
//!    a name is visible outside its package is decided by the case of
//!    its first letter. [`exported`] is where that is done.
//!
//! ## What is not here
//!
//! No function bodies, no statements, no channels, no goroutines. This
//! tree carries the *types* an IDL file becomes and the method
//! signatures an `interface` declares, and nothing else. Every form has
//! a renderer test, so no arm of `zerodds_lang_render::go` is code that
//! only claims to work.

use crate::common::{BodyText, Doc, Ident, Literal};
use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// A Go source file.
///
/// One file per IDL file, in one package. Go's unit of scope is the
/// package and a package is a directory, so nested IDL modules cannot
/// nest here — they flatten onto the declared names, which is what
/// `zerodds_lang_rules::adapter::go` does and what the C column does for
/// the same reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct File {
    /// The leading comment, written above the package clause.
    pub doc: Doc,
    /// The package name. Required: a Go file without one does not parse.
    pub package: Ident,
    /// `import` lines, in written order.
    pub imports: Vec<Import>,
    /// Declarations, in written order.
    pub decls: Vec<Decl>,
}

/// A file declaring the package a generated file always declares.
///
/// Written out rather than derived, because [`Ident`] has no `Default`
/// on purpose: an empty package name is not a Go file, and a derive
/// would produce one.
impl Default for File {
    fn default() -> Self {
        Self {
            doc: Doc::none(),
            package: Ident::new("zdtypes"),
            imports: Vec::new(),
            decls: Vec::new(),
        }
    }
}

/// One `import` line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Import {
    /// The import path, between the quotes.
    pub path: String,
    /// The name it is bound to, when it is not the path's last segment.
    pub alias: Option<Ident>,
}

/// One top-level declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decl {
    /// `type Name ...` or `type Name = ...`.
    Type(TypeDecl),
    /// `const Name Type = value`.
    Const(ConstDecl),
    /// `type Name interface { ... }`.
    ///
    /// Its own declaration rather than a [`Type`] inside a
    /// [`TypeDecl`], because a method set is the only thing in this
    /// tree that has a name, a parameter list and a result list, and
    /// putting it in [`Type`] would let a rule write a method set where
    /// a field's type belongs.
    Interface(InterfaceDecl),
    /// `func Name(params) results { ... }`.
    ///
    /// What a codec routine is. The types column emitted none of these;
    /// the statement layer below exists for exactly this declaration.
    Func(FuncDecl),
    /// A comment standing on its own.
    Comment(Doc),
    /// `@verbatim` text, copied into the output unchanged — OMG IDL 4.2
    /// §8.3.5.1 asks for the text "verbatim", so raw lines and never a
    /// comment.
    Verbatim(String),
}

/// `func Name(params) results { body }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuncDecl {
    /// Documentation.
    pub doc: Doc,
    /// The name declared. Lower-case first letter for a function only
    /// this package calls, which is what a generated codec is.
    pub name: Ident,
    /// Parameters, in order.
    pub params: Vec<Param>,
    /// Results, in order. Empty for a function that answers nothing.
    pub results: Vec<Type>,
    /// The body.
    pub body: Vec<Stmt>,
}

/// `type Name T` or `type Name = T`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeDecl {
    /// Documentation.
    pub doc: Doc,
    /// The name declared.
    pub name: Ident,
    /// Whether it is an alias — `type Name = T` — rather than a defined
    /// type.
    ///
    /// The two are not interchangeable: a defined type has its own
    /// identity and converts to its underlying type only explicitly.
    pub alias: bool,
    /// What it names.
    pub type_: Type,
    /// `@verbatim` text at the start and the end of a struct's body.
    /// Empty for any other type: only a struct has a body to hold it.
    pub body_text: BodyText,
}

/// `const Name Type = value`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstDecl {
    /// Documentation.
    pub doc: Doc,
    /// The name declared.
    pub name: Ident,
    /// The declared type, absent for an untyped constant.
    pub type_: Option<Type>,
    /// The value.
    pub value: Expr,
}

/// `type Name interface { ... }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceDecl {
    /// Documentation.
    pub doc: Doc,
    /// The name declared.
    pub name: Ident,
    /// The method set, in written order.
    pub methods: Vec<Method>,
}

/// One method of an interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Method {
    /// Documentation.
    pub doc: Doc,
    /// The name.
    pub name: Ident,
    /// Parameters, in order.
    pub params: Vec<Param>,
    /// Results, in order. Empty for an operation returning `void`.
    pub results: Vec<Type>,
}

/// One parameter of a method.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    /// The name, absent where only the type is written.
    pub name: Option<Ident>,
    /// The type.
    pub type_: Type,
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
    /// The struct tag, when the field carries one.
    ///
    /// Go's own mechanism for text a program reads back with
    /// `reflect.StructTag.Get`. The IDL spelling of a member goes here:
    /// a member's ID under `@autoid(HASH)` and under `@hashid` is the
    /// first four octets of the MD5 of its **IDL** name
    /// (DDS-XTypes 1.3 §7.3.1.2.1.1), and the Go spelling has been moved
    /// by [`exported`] and by [`escape_name`], neither of which inverts.
    pub tag: Option<Tag>,
}

/// A struct tag: `key:"value"` pairs, in written order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    /// The pairs, in written order.
    pub entries: Vec<(String, String)>,
}

impl Tag {
    /// A tag carrying one pair.
    #[must_use]
    pub fn one(key: &str, value: &str) -> Self {
        Self {
            entries: alloc::vec![(key.to_string(), value.to_string())],
        }
    }
}

/// A Go type, as it will be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    /// A built-in.
    Primitive(Primitive),
    /// A declared name.
    Named(Ident),
    /// `[]T`.
    Slice(Box<Type>),
    /// `[N]T`.
    Array {
        /// The length. Part of the type in Go, as it is in IDL.
        length: Box<Expr>,
        /// The element type.
        element: Box<Type>,
    },
    /// `*T`.
    Pointer(Box<Type>),
    /// `struct { ... }`.
    Struct(Vec<Field>),
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
        Self::Slice(Box::new(element))
    }

    /// `*T`.
    #[must_use]
    pub fn pointer(target: Self) -> Self {
        Self::Pointer(Box::new(target))
    }

    /// `[n]T`.
    #[must_use]
    pub fn array(length: i128, element: Self) -> Self {
        Self::Array {
            length: Box::new(Expr::Literal(Literal::int(length))),
            element: Box::new(element),
        }
    }
}

/// A Go predeclared type.
///
/// `int`, `uint` and `uintptr` are **not** here and that is the point:
/// their width is implementation-specific (Go spec, "Numeric types"), so
/// a generated declaration using one would have a different size on a
/// 32-bit target than on a 64-bit one, for a wire format that fixes it.
/// `complex64` and `complex128` are absent because no IDL construct
/// reaches them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Primitive {
    /// `bool`
    Bool,
    /// `int8`
    Int8,
    /// `int16`
    Int16,
    /// `int32`
    Int32,
    /// `int64`
    Int64,
    /// `uint8`
    UInt8,
    /// `uint16`
    UInt16,
    /// `uint32`
    UInt32,
    /// `uint64`
    UInt64,
    /// `float32`
    Float32,
    /// `float64`
    Float64,
    /// `string`
    String,
    /// `any` — the empty interface, spelled as the Go 1.18 alias.
    Any,
}

impl Primitive {
    /// Every built-in the tree can hold.
    pub const ALL: [Self; 13] = [
        Self::Bool,
        Self::Int8,
        Self::Int16,
        Self::Int32,
        Self::Int64,
        Self::UInt8,
        Self::UInt16,
        Self::UInt32,
        Self::UInt64,
        Self::Float32,
        Self::Float64,
        Self::String,
        Self::Any,
    ];

    /// How it is written.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::Int8 => "int8",
            Self::Int16 => "int16",
            Self::Int32 => "int32",
            Self::Int64 => "int64",
            Self::UInt8 => "uint8",
            Self::UInt16 => "uint16",
            Self::UInt32 => "uint32",
            Self::UInt64 => "uint64",
            Self::Float32 => "float32",
            Self::Float64 => "float64",
            Self::String => "string",
            Self::Any => "any",
        }
    }
}

/// An expression.
///
/// Only the forms a constant declaration and an array length take. Go
/// requires an array length to be a constant expression, which is why
/// [`Expr::Binary`] is here at all: a bitmask constant is `1 << 4`.
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
    /// `T(v)` — a conversion.
    ///
    /// Go has no implicit conversion between named integer types, so a
    /// constant of a declared enumeration type is written this way and
    /// not as a bare number.
    Convert {
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
    /// call is a call on a selector.
    Call {
        /// What is called.
        func: Box<Expr>,
        /// The arguments, in order.
        args: Vec<Expr>,
    },
    /// `value.name` — a field or method selection.
    Selector {
        /// The value selected into.
        value: Box<Expr>,
        /// The name selected.
        name: Ident,
    },
    /// `value[index]`.
    Index {
        /// The value indexed.
        value: Box<Expr>,
        /// The index.
        index: Box<Expr>,
    },
    /// `value[:high]` — a slice expression keeping the first elements.
    SliceTo {
        /// The value sliced.
        value: Box<Expr>,
        /// The exclusive end.
        high: Box<Expr>,
    },
    /// `op value` — `!`, `-`, `^` (Go's bitwise complement), `*` and
    /// `&`, written as given with no space.
    Unary {
        /// The operator, as written.
        op: &'static str,
        /// The operand.
        value: Box<Expr>,
    },
    /// `T{...}` — a composite literal, keyed or positional. The type is
    /// absent for a nested literal whose type the enclosing one already
    /// states, which is the elision `gofmt -s` asks for and this
    /// generator writes directly.
    Composite {
        /// The literal's type, absent where elided.
        type_: Option<Box<Type>>,
        /// The elements, in written order.
        elems: Vec<CompositeElem>,
    },
    /// `make([]T, length)`.
    Make {
        /// The slice type made.
        type_: Box<Type>,
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

    /// A bare name.
    #[must_use]
    pub fn name(text: &str) -> Self {
        Self::Name(Ident::new(text))
    }

    /// `value.name`.
    #[must_use]
    pub fn selector(value: Self, name: &str) -> Self {
        Self::Selector {
            value: Box::new(value),
            name: Ident::new(name),
        }
    }

    /// `func(args)`.
    #[must_use]
    pub fn call(func: Self, args: Vec<Self>) -> Self {
        Self::Call {
            func: Box::new(func),
            args,
        }
    }
}

/// One element of a composite literal, keyed or positional.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompositeElem {
    /// The field name, absent for a positional element.
    pub key: Option<Ident>,
    /// The value.
    pub value: Expr,
}

/// A statement, as a codec body writes one.
///
/// Only the forms the shared codec walk emits. Go's own richer set —
/// `switch`, `range`, `defer`, channels — is deliberately not here: a
/// form no walk builds is a renderer arm no test reaches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    /// `var name T = value`, `var name T`, or `name := value` where the
    /// type is inferred.
    Var {
        /// The name declared.
        name: Ident,
        /// The declared type; absent asks for `:=` inference and then a
        /// value is required.
        type_: Option<Type>,
        /// The initialiser.
        value: Option<Expr>,
    },
    /// `place = value`.
    Assign {
        /// The place written.
        place: Expr,
        /// The value.
        value: Expr,
    },
    /// An expression evaluated for its effect, which in Go is a call.
    Expr(Expr),
    /// `return`, with or without a value.
    Return(Option<Expr>),
    /// `if condition { .. } else { .. }`.
    If {
        /// The condition.
        condition: Expr,
        /// The then-branch.
        then: Vec<Stmt>,
        /// The else-branch, absent where there is none.
        otherwise: Option<Vec<Stmt>>,
    },
    /// `for counter := init; counter < bound; counter++ { .. }`.
    ///
    /// The init expression is the caller's because it carries the
    /// counter's type — `uint32(0)` where the bound is a `uint32` — and
    /// a tree that hard-coded one would decide a typing question the
    /// backend owns.
    For {
        /// The counter's name.
        counter: Ident,
        /// What the counter starts as.
        init: Box<Expr>,
        /// The exclusive bound.
        bound: Box<Expr>,
        /// The body.
        body: Vec<Stmt>,
    },
    /// `for condition { .. }` — a loop with no counter.
    While {
        /// The condition.
        condition: Expr,
        /// The body.
        body: Vec<Stmt>,
    },
    /// `break`.
    Break,
    /// A comment on its own line.
    Comment(Doc),
}

/// The name an IDL identifier takes in Go, before it is exported.
///
/// A trailing underscore where the identifier is a Go keyword or one of
/// the predeclared names a generated file relies on.
///
/// The keyword list is the Go spec's twenty-five ("Keywords"). The rest
/// are the predeclared identifiers ("Predeclared identifiers"), which are
/// *not* keywords — Go lets a declaration shadow `string` or `len`, and
/// the file still compiles until a later declaration wants the original.
/// A generated file wants the originals: every field type in it is one of
/// `bool`, `int32`, `string` and the rest, so a type declared `String`
/// would be reached ahead of the predeclared one in the same package.
///
/// The IDL name is not lost when this fires: it is written into the
/// field's [`Tag`] and into a comment above every other kind of
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

/// The name a declaration takes so that a package outside this one can
/// reach it.
///
/// Go has no `public` keyword. A declared name is exported if and only
/// if its first character is an upper-case letter (Go spec, "Exported
/// identifiers"), so the case of one letter is what
/// [`crate::common::Visibility::Public`] means here — and a generated
/// type nothing outside the package can name is a types column with no
/// consumer.
///
/// Only the first character moves, and only when it has an upper case at
/// all. A leading underscore is left alone rather than replaced: `_d` is
/// not a name Go can export, and inventing one would make the Go
/// spelling differ from the IDL for a member the wire format names.
/// [`crate::common::Ident`] never renames on its own, so the caller that
/// wants the exported spelling asks for it — which is
/// `zerodds_lang_rules::adapter::go`, once, where the collision this can
/// cause is also checked.
#[must_use]
pub fn exported(name: &str) -> String {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    if !first.is_lowercase() {
        return name.to_string();
    }
    let mut out = String::new();
    for upper in first.to_uppercase() {
        out.push(upper);
    }
    out.push_str(chars.as_str());
    out
}

/// Every name an escape has to move out of the way of.
///
/// The twenty-five keywords and the predeclared identifiers, including
/// the ones added in Go 1.18 (`any`) and Go 1.21 (`min`, `max`,
/// `clear`). A generated file that shadowed one of these would compile
/// until a sibling declaration needed the original, which is a failure
/// that arrives in a file nobody changed.
pub const RESERVED: &[&str] = &[
    // Keywords, Go spec "Keywords".
    "break",
    "case",
    "chan",
    "const",
    "continue",
    "default",
    "defer",
    "else",
    "fallthrough",
    "for",
    "func",
    "go",
    "goto",
    "if",
    "import",
    "interface",
    "map",
    "package",
    "range",
    "return",
    "select",
    "struct",
    "switch",
    "type",
    "var",
    // Predeclared types.
    "any",
    "bool",
    "byte",
    "comparable",
    "complex64",
    "complex128",
    "error",
    "float32",
    "float64",
    "int",
    "int8",
    "int16",
    "int32",
    "int64",
    "rune",
    "string",
    "uint",
    "uint8",
    "uint16",
    "uint32",
    "uint64",
    "uintptr",
    // Predeclared constants and the zero value.
    "true",
    "false",
    "iota",
    "nil",
    // Predeclared functions.
    "append",
    "cap",
    "clear",
    "close",
    "complex",
    "copy",
    "delete",
    "imag",
    "len",
    "make",
    "max",
    "min",
    "new",
    "panic",
    "print",
    "println",
    "real",
    "recover",
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
    fn no_primitive_has_an_implementation_defined_width() {
        // `int`, `uint` and `uintptr` are 32 or 64 bits depending on the
        // target (Go spec, "Numeric types"). A member declared with one
        // would change size between a build for amd64 and one for a
        // 32-bit ARM, for a wire format that fixes every width.
        for primitive in Primitive::ALL {
            assert!(
                !matches!(primitive.as_str(), "int" | "uint" | "uintptr"),
                "{} has no fixed width",
                primitive.as_str()
            );
        }
    }

    #[test]
    fn a_keyword_escapes_with_a_trailing_underscore() {
        assert_eq!(escape_name("type"), "type_");
        assert_eq!(escape_name("range"), "range_");
        assert_eq!(escape_name("payload"), "payload");
    }

    #[test]
    fn a_predeclared_name_escapes_although_it_would_parse() {
        // Go permits the shadowing; a generated file cannot afford it,
        // because every field type in the same file is one of these.
        for name in ["string", "int32", "len", "any", "error", "min"] {
            assert_eq!(escape_name(name), alloc::format!("{name}_"));
        }
    }

    #[test]
    fn exporting_moves_the_first_letter_and_nothing_else() {
        assert_eq!(exported("value"), "Value");
        assert_eq!(exported("Value"), "Value");
        assert_eq!(exported("aBc"), "ABc");
        assert_eq!(exported(""), "");
    }

    #[test]
    fn a_leading_underscore_is_left_alone_because_go_cannot_export_it() {
        // `_d` is the discriminator's name in every column. Go cannot
        // export a name starting with an underscore at all, and giving
        // it a different one here would make the Go spelling differ from
        // the IDL for a member the wire format names.
        assert_eq!(exported("_d"), "_d");
        assert_eq!(exported("_reserved"), "_reserved");
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
    fn an_array_type_nests_outermost_dimension_first() {
        // `long grid[2][3]` is two rows of three, which in Go is
        // `[2][3]int32`. Reversing it silently transposes the data.
        let type_ = Type::array(2, Type::array(3, Type::Primitive(Primitive::Int32)));
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
    fn a_field_carries_its_idl_spelling_in_a_tag() {
        let field = Field {
            doc: Doc::none(),
            name: Ident::new("Type_"),
            type_: Type::Primitive(Primitive::Int32),
            tag: Some(Tag::one("idl", "type")),
        };
        let tag = field.tag.expect("the tag");
        assert_eq!(tag.entries.len(), 1);
        assert_eq!(tag.entries[0].1, "type");
    }
}
