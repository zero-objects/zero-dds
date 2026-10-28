// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Rust concrete syntax tree. Spec §1.
//!
//! Concrete rather than abstract: a derive is a node in a recorded
//! order, not a flag the renderer expands. A rule that adds `Clone` is
//! then visible as a rule that adds `Clone`.

use crate::common::{BodyText, Doc, Ident, Literal, Path, Visibility};
use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// A Rust source file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct File {
    /// Inner attributes, `#![...]`, in written order.
    pub inner_attrs: Vec<Attr>,
    /// The file's own documentation.
    pub doc: Doc,
    /// `use` declarations, in written order.
    pub uses: Vec<Use>,
    /// Items, in written order.
    pub items: Vec<Item>,
}

/// A `use` declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Use {
    /// The path.
    pub path: Path,
    /// The names brought in from it, empty for a plain `use path;`.
    pub names: Vec<Ident>,
    /// Whether it is `pub use`.
    pub visibility: Visibility,
}

/// One item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    /// `mod name { ... }`
    Module(Module),
    /// `struct Name { ... }`
    Struct(Struct),
    /// `enum Name { ... }`
    Enum(Enum),
    /// `type Name = ...;`
    TypeAlias(TypeAlias),
    /// `const NAME: T = ...;`
    Const(Const),
    /// `trait Name { ... }`
    Trait(Trait),
    /// `impl ... { ... }`
    Impl(Impl),
    /// `fn name(...) -> ... { ... }`
    Function(Function),
    /// `@verbatim` text, copied into the output unchanged — OMG IDL 4.2
    /// §8.3.5.1 asks for the text "verbatim", so raw lines and never a
    /// comment.
    Verbatim(String),
}

/// `mod name { ... }`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module {
    /// Documentation.
    pub doc: Doc,
    /// Outer attributes.
    pub attrs: Vec<Attr>,
    /// Visibility.
    pub visibility: Visibility,
    /// Name.
    pub name: Ident,
    /// `use` declarations inside it, in written order.
    ///
    /// A module is its own scope, so a generated one that names a type
    /// from outside needs the import here and not only at the top of the
    /// file. Without it, nesting a codec by its IDL module put `Writer`
    /// out of reach and the generated file did not compile.
    pub uses: Vec<Use>,
    /// Items inside it.
    pub items: Vec<Item>,
}

/// `struct Name { ... }`, in any of its three forms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Struct {
    /// Documentation.
    pub doc: Doc,
    /// Outer attributes, `#[...]`, in written order.
    pub attrs: Vec<Attr>,
    /// Derives, in written order — one `#[derive(..)]` listing them.
    pub derives: Vec<Derive>,
    /// Visibility.
    pub visibility: Visibility,
    /// Name.
    pub name: Ident,
    /// Generic parameters.
    pub generics: Vec<GenericParam>,
    /// The body.
    pub body: StructBody,
    /// `@verbatim` text at the start and the end of a braced body. A
    /// unit struct that carries some is written with braces.
    pub body_text: BodyText,
}

/// Which of the three struct forms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructBody {
    /// `struct S { a: T }`
    Named(Vec<Field>),
    /// `struct S(T);`
    Tuple(Vec<Type>),
    /// `struct S;`
    Unit,
}

/// One named field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// Documentation.
    pub doc: Doc,
    /// Outer attributes.
    pub attrs: Vec<Attr>,
    /// Visibility.
    pub visibility: Visibility,
    /// Name.
    pub name: Ident,
    /// Type.
    pub type_: Type,
}

/// `enum Name { ... }`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Enum {
    /// Documentation.
    pub doc: Doc,
    /// Outer attributes.
    pub attrs: Vec<Attr>,
    /// Derives.
    pub derives: Vec<Derive>,
    /// Visibility.
    pub visibility: Visibility,
    /// Name.
    pub name: Ident,
    /// Generic parameters.
    pub generics: Vec<GenericParam>,
    /// Variants, in written order.
    pub variants: Vec<Variant>,
}

/// One enum variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variant {
    /// Documentation.
    pub doc: Doc,
    /// Outer attributes.
    pub attrs: Vec<Attr>,
    /// Name.
    pub name: Ident,
    /// Payload.
    pub body: VariantBody,
    /// `= <expr>`, for a C-like enum.
    pub discriminant: Option<Expr>,
}

/// What a variant carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VariantBody {
    /// `Name`
    Unit,
    /// `Name(T, U)`
    Tuple(Vec<Type>),
    /// `Name { a: T }`
    Named(Vec<Field>),
}

/// `type Name<..> = ..;`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeAlias {
    /// Documentation.
    pub doc: Doc,
    /// Outer attributes.
    pub attrs: Vec<Attr>,
    /// Visibility.
    pub visibility: Visibility,
    /// Name.
    pub name: Ident,
    /// Generic parameters.
    pub generics: Vec<GenericParam>,
    /// What it aliases.
    pub type_: Type,
}

/// `const NAME: T = expr;`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Const {
    /// Documentation.
    pub doc: Doc,
    /// Outer attributes.
    pub attrs: Vec<Attr>,
    /// Visibility.
    pub visibility: Visibility,
    /// Name.
    pub name: Ident,
    /// Type.
    pub type_: Type,
    /// Value.
    pub value: Expr,
}

/// `trait Name { ... }`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trait {
    /// Documentation.
    pub doc: Doc,
    /// Outer attributes.
    pub attrs: Vec<Attr>,
    /// Visibility.
    pub visibility: Visibility,
    /// Name.
    pub name: Ident,
    /// Generic parameters.
    pub generics: Vec<GenericParam>,
    /// Supertraits.
    pub supertraits: Vec<Type>,
    /// Items, which for a trait are signatures and associated types.
    pub items: Vec<Item>,
}

/// `impl Trait for Type { ... }`, or an inherent `impl Type { ... }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Impl {
    /// Documentation.
    pub doc: Doc,
    /// Outer attributes.
    pub attrs: Vec<Attr>,
    /// Generic parameters.
    pub generics: Vec<GenericParam>,
    /// The trait implemented, absent for an inherent impl.
    pub trait_: Option<Type>,
    /// The type implemented for.
    pub self_type: Type,
    /// Items inside.
    pub items: Vec<Item>,
}

/// `fn name(..) -> .. { .. }`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    /// Documentation.
    pub doc: Doc,
    /// Outer attributes.
    pub attrs: Vec<Attr>,
    /// Visibility.
    pub visibility: Visibility,
    /// Name.
    pub name: Ident,
    /// Generic parameters.
    pub generics: Vec<GenericParam>,
    /// Parameters, in order.
    pub params: Vec<Param>,
    /// Return type, absent for `()`.
    pub returns: Option<Type>,
    /// The body, absent for a trait signature.
    pub body: Option<Block>,
}

/// One function parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    /// The name, absent for a receiver.
    pub name: Option<Ident>,
    /// The type.
    pub type_: Type,
    /// Whether it is a receiver — `self`, `&self`, `&mut self`.
    pub receiver: Option<Receiver>,
}

/// Which receiver form a parameter is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Receiver {
    /// `self`
    Value,
    /// `&self`
    Ref,
    /// `&mut self`
    RefMut,
}

/// One generic parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericParam {
    /// Name.
    pub name: Ident,
    /// Bounds, in written order.
    pub bounds: Vec<Type>,
    /// Whether it is a lifetime rather than a type.
    pub lifetime: bool,
}

/// A type as it will be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    /// A named type, with any generic arguments.
    Named {
        /// Its path.
        path: Path,
        /// Its generic arguments, in written order.
        args: Vec<Type>,
    },
    /// `&T` or `&mut T`.
    Ref {
        /// The referent.
        target: Box<Type>,
        /// Whether it is mutable.
        mutable: bool,
        /// A named lifetime, if one is written.
        lifetime: Option<Ident>,
    },
    /// `[T; N]`
    Array {
        /// Element type.
        element: Box<Type>,
        /// Length, as an expression so `[u8; MAX]` stays what it says.
        length: Box<Expr>,
    },
    /// `[T]`
    Slice(Box<Type>),
    /// `(A, B)`, and `()` for the empty tuple.
    Tuple(Vec<Type>),
    /// A primitive: `u32`, `bool`, `f64`.
    Primitive(Primitive),
    /// `impl Trait` in return position.
    ImplTrait(Vec<Type>),
    /// `dyn Trait`.
    DynTrait(Vec<Type>),
}

impl Type {
    /// A named type with no generic arguments.
    #[must_use]
    pub fn named(path: Path) -> Self {
        Self::Named {
            path,
            args: Vec::new(),
        }
    }

    /// A named type with generic arguments.
    #[must_use]
    pub fn generic(path: Path, args: Vec<Self>) -> Self {
        Self::Named { path, args }
    }

    /// The unit type, `()`.
    #[must_use]
    pub fn unit() -> Self {
        Self::Tuple(Vec::new())
    }
}

/// A Rust primitive type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Primitive {
    /// `bool`
    Bool,
    /// `char`
    Char,
    /// `i8`
    I8,
    /// `i16`
    I16,
    /// `i32`
    I32,
    /// `i64`
    I64,
    /// `i128`
    I128,
    /// `u8`
    U8,
    /// `u16`
    U16,
    /// `u32`
    U32,
    /// `u64`
    U64,
    /// `u128`
    U128,
    /// `f32`
    F32,
    /// `f64`
    F64,
    /// `str`
    Str,
    /// `String`
    String,
}

impl Primitive {
    /// Every primitive the tree can hold.
    pub const ALL: [Self; 16] = [
        Self::Bool,
        Self::Char,
        Self::I8,
        Self::I16,
        Self::I32,
        Self::I64,
        Self::I128,
        Self::U8,
        Self::U16,
        Self::U32,
        Self::U64,
        Self::U128,
        Self::F32,
        Self::F64,
        Self::Str,
        Self::String,
    ];

    /// How it is written.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::Char => "char",
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::I128 => "i128",
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
            Self::U128 => "u128",
            Self::F32 => "f32",
            Self::F64 => "f64",
            Self::Str => "str",
            Self::String => "String",
        }
    }
}

/// An outer or inner attribute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attr {
    /// The attribute path: `serde`, `cfg`, `repr`.
    pub path: Path,
    /// Its arguments, absent for a bare `#[name]`.
    pub args: Vec<AttrArg>,
}

/// One argument of an attribute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttrArg {
    /// A bare word: `#[repr(C)]`.
    Word(Ident),
    /// `name = value`: `#[serde(rename = "x")]`.
    Pair {
        /// The name.
        name: Ident,
        /// The value.
        value: Literal,
    },
    /// A nested list: `#[cfg(all(unix, feature = "x"))]`.
    Nested {
        /// The name.
        name: Ident,
        /// Its own arguments.
        args: Vec<AttrArg>,
    },
}

/// One derive, recorded as its own node.
///
/// Not a flag: the order they appear in is the order they were added, and
/// a rule that adds one is visible as a rule that adds one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Derive {
    /// The trait derived.
    pub path: Path,
}

impl Derive {
    /// A derive of a trait named by one segment.
    #[must_use]
    pub fn of(name: &str) -> Self {
        Self {
            path: Path::single(name),
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
    /// `let name: T = expr;`
    Let {
        /// Name.
        name: Ident,
        /// Type, when written.
        type_: Option<Type>,
        /// Initialiser.
        value: Option<Expr>,
        /// Whether it is `let mut`.
        mutable: bool,
    },
    /// An expression followed by `;`.
    Semi(Expr),
    /// A trailing expression with no `;`, which is the block's value.
    Tail(Expr),
    /// An item declared inside a block.
    Item(Box<Item>),
}

/// An expression.
///
/// Enough to write the bodies a generator produces — constructing a
/// value, calling a method, matching a variant — and no more. An
/// expression form nothing generates is a form nothing can review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    /// A literal.
    Literal(Literal),
    /// A path: a name, a constant, a unit variant.
    Path(Path),
    /// `receiver.method(args)`
    MethodCall {
        /// What it is called on.
        receiver: Box<Expr>,
        /// The method.
        method: Ident,
        /// Its arguments.
        args: Vec<Expr>,
    },
    /// `path(args)`
    Call {
        /// What is called.
        callee: Box<Expr>,
        /// Its arguments.
        args: Vec<Expr>,
    },
    /// `Type { field: value, .. }`
    StructLit {
        /// The type constructed.
        path: Path,
        /// Its fields, in written order.
        fields: Vec<(Ident, Expr)>,
        /// `..base`, when written.
        base: Option<Box<Expr>>,
    },
    /// `receiver.field`
    Field {
        /// What it is taken from.
        receiver: Box<Expr>,
        /// The field.
        name: Ident,
    },
    /// `&expr` or `&mut expr`.
    Ref {
        /// What is referenced.
        target: Box<Expr>,
        /// Whether it is mutable.
        mutable: bool,
    },
    /// `expr?`
    Try(Box<Expr>),
    /// `|a, b| body`
    ///
    /// One expression body, not a block: the generator only ever needs
    /// a closure to adapt an argument order, and a closure with
    /// statements in it would be a function that should have a name.
    Closure {
        /// The parameters, in order and untyped — inference has them
        /// from the call site.
        params: Vec<Ident>,
        /// The body.
        body: Box<Expr>,
    },
    /// `match scrutinee { arms }`
    Match {
        /// What is matched.
        scrutinee: Box<Expr>,
        /// The arms, in written order.
        arms: Vec<MatchArm>,
    },
    /// `if cond { .. } else { .. }`
    If {
        /// The condition.
        condition: Box<Expr>,
        /// The then-block.
        then: Block,
        /// The else-block, when written.
        otherwise: Option<Block>,
    },
    /// `for name in iter { .. }`
    For {
        /// The binding.
        binding: Ident,
        /// What is iterated.
        iterator: Box<Expr>,
        /// The body.
        body: Block,
    },
    /// `while cond { .. }`
    ///
    /// A header-driven decoder reads until the delimiter's extent is
    /// used up or the parameter list ends, and neither bound is a count
    /// a `for` could iterate. The C column already had one; the Rust
    /// column walked its plan instead, which is the same thing only
    /// when the writer's members arrive in the reader's order.
    While {
        /// The condition, tested before each pass.
        condition: Box<Expr>,
        /// The body.
        body: Block,
    },
    /// `place = value`
    ///
    /// Only what a loop needs: a slot the body fills once it has read
    /// the header that says which slot it is.
    Assign {
        /// Where it goes.
        target: Box<Expr>,
        /// What goes there.
        value: Box<Expr>,
    },
    /// `return expr;`
    Return(Option<Box<Expr>>),
    /// `[a, b, c]`
    Array(Vec<Expr>),
    /// `[value; count]`
    ///
    /// The form a default value for an array takes. Writing the elements
    /// out would put a thousand copies of a struct literal in a
    /// generated file for `long grid[1000]`.
    Repeat {
        /// The element.
        value: Box<Expr>,
        /// How many.
        count: Box<Expr>,
    },
    /// `(a, b)`
    Tuple(Vec<Expr>),
    /// A binary operation.
    Binary {
        /// The operator, as written.
        op: BinaryOp,
        /// Left.
        lhs: Box<Expr>,
        /// Right.
        rhs: Box<Expr>,
    },
    /// `!expr`
    ///
    /// A decoder that has just read a flag and has to refuse when it is
    /// clear. Writing it as `== false` renders, and reads as a mistake
    /// in the output.
    Not(Box<Expr>),
    /// `*expr`
    ///
    /// The second unary operator the generated bodies need. A borrow is
    /// what a match arm over `&Option<T>` binds and what an element
    /// closure receives, and the runtime takes a primitive by value —
    /// so `*v` is the only way to hand one over. Emitting `v` instead
    /// would not compile, which is loud, but writing the closure at all
    /// requires this form to exist.
    Deref(Box<Expr>),
    /// `expr as T`
    Cast {
        /// What is cast.
        value: Box<Expr>,
        /// The target type.
        type_: Box<Type>,
    },
    /// A block used as an expression.
    Block(Block),
}

/// A binary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinaryOp {
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// `/`
    Div,
    /// `%`
    Rem,
    /// `==`
    Eq,
    /// `!=`
    Ne,
    /// `<`
    Lt,
    /// `<=`
    Le,
    /// `>`
    Gt,
    /// `>=`
    Ge,
    /// `&&`
    And,
    /// `||`
    Or,
    /// `&`
    BitAnd,
    /// `|`
    BitOr,
    /// `^`
    BitXor,
    /// `<<`
    Shl,
    /// `>>`
    Shr,
}

impl BinaryOp {
    /// How it is written.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Add => "+",
            Self::Sub => "-",
            Self::Mul => "*",
            Self::Div => "/",
            Self::Rem => "%",
            Self::Eq => "==",
            Self::Ne => "!=",
            Self::Lt => "<",
            Self::Le => "<=",
            Self::Gt => ">",
            Self::Ge => ">=",
            Self::And => "&&",
            Self::Or => "||",
            Self::BitAnd => "&",
            Self::BitOr => "|",
            Self::BitXor => "^",
            Self::Shl => "<<",
            Self::Shr => ">>",
        }
    }
}

/// One arm of a `match`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchArm {
    /// The pattern.
    pub pattern: Pattern,
    /// A guard, when written.
    pub guard: Option<Expr>,
    /// The body.
    pub body: Expr,
}

/// A pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pattern {
    /// `_`
    Wildcard,
    /// A binding by name.
    Binding(Ident),
    /// A literal pattern.
    Literal(Literal),
    /// A path pattern, for a unit variant or a constant.
    Path(Path),
    /// `Path(a, b)`
    TupleStruct {
        /// The path.
        path: Path,
        /// Its elements.
        elements: Vec<Pattern>,
    },
    /// `Path { a, b }`
    Struct {
        /// The path.
        path: Path,
        /// Its fields.
        fields: Vec<(Ident, Pattern)>,
        /// Whether it ends in `..`.
        rest: bool,
    },
}

/// The name a Rust keyword collision escapes to.
///
/// `r#type` rather than `type_`: the raw form keeps the name the IDL
/// wrote, and a reader of the generated code sees the field it declared.
#[must_use]
pub fn escape_keyword(name: &str) -> String {
    if RESERVED.contains(&name) {
        let mut out = String::from("r#");
        out.push_str(name);
        return out;
    }
    name.to_string()
}

/// The Rust keywords a generated identifier may collide with.
///
/// The strict and reserved keywords of the 2021 edition, less the ones
/// no raw identifier is permitted for.
const RESERVED: &[&str] = &[
    "as", "break", "const", "continue", "else", "enum", "extern", "false", "fn", "for", "if",
    "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return", "static",
    "struct", "trait", "true", "type", "unsafe", "use", "where", "while", "async", "await", "dyn",
    "abstract", "become", "box", "do", "final", "macro", "override", "priv", "typeof", "unsized",
    "virtual", "yield", "try",
];

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn a_keyword_escapes_to_the_raw_form_and_keeps_the_name() {
        // `r#type` rather than `type_`: a reader of the generated code
        // sees the field the IDL declared.
        assert_eq!(escape_keyword("type"), "r#type");
        assert_eq!(escape_keyword("match"), "r#match");
        assert_eq!(escape_keyword("value"), "value");
    }

    #[test]
    fn self_and_crate_are_not_escapable_and_are_not_in_the_table() {
        // No raw identifier exists for them, so escaping would produce
        // something that does not compile.
        for name in ["self", "Self", "super", "crate"] {
            assert!(!RESERVED.contains(&name), "{name} is in the table");
        }
    }

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
    fn derives_keep_the_order_they_were_added_in() {
        // Order is not cosmetic: it is what a diff of two rule
        // catalogues compares.
        let s = Struct {
            body_text: crate::common::BodyText::none(),
            doc: Doc::none(),
            attrs: Vec::new(),
            derives: alloc::vec![Derive::of("Debug"), Derive::of("Clone")],
            visibility: Visibility::Public,
            name: Ident::new("Pose"),
            generics: Vec::new(),
            body: StructBody::Named(Vec::new()),
        };
        let names: Vec<&str> = s
            .derives
            .iter()
            .filter_map(|d| d.path.last().map(Ident::as_str))
            .collect();
        assert_eq!(names, alloc::vec!["Debug", "Clone"]);
    }

    #[test]
    fn an_array_length_is_an_expression_not_a_number() {
        // `[u8; MAX]` has to stay what it says; folding it to a number
        // loses the constant the IDL named.
        let type_ = Type::Array {
            element: alloc::boxed::Box::new(Type::Primitive(Primitive::U8)),
            length: alloc::boxed::Box::new(Expr::Path(Path::single("MAX"))),
        };
        let Type::Array { length, .. } = type_ else {
            panic!("expected an array");
        };
        assert!(matches!(*length, Expr::Path(_)));
    }
}
