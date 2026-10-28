// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Kotlin concrete syntax tree. Spec §1.
//!
//! Kotlin shares the JVM's value model with [`crate::java`] — no
//! unsigned types, a `Char` is one UTF-16 code unit, an object field is
//! already a reference and needs no pointer form — so the two trees
//! answer the same wire questions the same way, and reading them side
//! by side is the point. What differs is the surface Kotlin itself
//! offers, and this tree uses it where it is simpler:
//!
//! - **A `.kt` file is not one public type.** Java restricts a file to
//!   one public top-level type; Kotlin has none of that (Kotlin
//!   language specification, "Kotlin files"), so [`File::decls`] holds
//!   top-level declarations directly — a generated type, a generated
//!   codec function, a top-level constant — with no wrapper class the
//!   Java column needs one of just to hold its `static` methods.
//! - **A codec routine is a top-level `fun`,** as it is in
//!   [`crate::zig`], [`crate::go`] and [`crate::d`], and not a `static`
//!   method on an invented holder class.
//! - **An enumeration is not Kotlin's `enum class`.** `enum class`
//!   fixes one value per constant to the declaration's own ordinal
//!   sequence; DDS-XTypes 1.3 §7.4.3.5.3 admits an enumerator's
//!   `@value` with a gap or an out-of-order number, which an ordinal
//!   cannot hold without renumbering the wire. [`Decl::Object`] declares
//!   the enumeration's constants as a Kotlin `object` of `const val`
//!   fields instead — the same reading `adapter/java.rs` gives for the
//!   identical reason, spelled as a singleton rather than as a class of
//!   `static final` fields because that is what an aggregate of
//!   constants with no instance is in Kotlin.
//! - **A struct, a union, a bitset, an exception and a valuetype are all
//!   [`Decl::Class`] as a `data class`.** Every property carries a
//!   default, which is what lets `crate::emit::kotlin`'s `storage`
//!   call the zero-argument constructor Kotlin then generates for it
//!   (Kotlin language specification, "Data classes"); the sibling Java
//!   column needs a separate declared no-argument constructor for the
//!   same reason and Kotlin needs none.
//!
//! ## The memory shapes
//!
//! | IDL | Kotlin |
//! |---|---|
//! | `boolean` | `Boolean` |
//! | `octet`, `uint8` | `Int` (Kotlin has no unsigned `Byte`) |
//! | `char` | `Char` |
//! | `int8` | `Byte` |
//! | `short` | `Short` |
//! | `unsigned short`, `wchar` | `Int` |
//! | `long` | `Int` |
//! | `unsigned long` | `Long` |
//! | `long long`, `unsigned long long` | `Long` (the pattern, for the unsigned one) |
//! | `float` | `Float` |
//! | `double` | `Double` |
//! | `long double` | `ZdWire.Float128`, a pair of `Long`s |
//! | `string`, `wstring`, `fixed<P,S>` | `String` |
//! | `sequence<T>` | `Array<T>`, or `TArray` for a primitive `T` |
//! | `T[N][M]` | `Array<Array<T>>`, outer dimension outermost |
//! | `map<K,V>` | `Array` of a generated entry class |
//! | `@optional T m` | `T?`, `null` when absent |
//! | `@external T m` | `T` — every JVM field is already a reference |
//! | `struct`, `valuetype`, `exception`, `bitset` | `data class` |
//! | `union` | `data class` with a `_d` property, one property per branch |
//! | `enum` | an `object` of `const val Int` |
//! | `bitmask` | a `typealias` to `Int` or `Long`, and `const val` bits |
//! | `typedef` | a `typealias` |
//! | `interface` | a `class` of function-typed properties |
//!
//! ## No visibility field
//!
//! Kotlin's top-level default visibility is `public` (Kotlin language
//! specification, "Declarations"), and a generated file exists to be
//! imported, so nothing here writes a modifier for it — the same reason
//! [`crate::zig::ConstDecl::is_pub`] is stated explicitly rather than
//! assumed does not apply: Zig's default is file-private and has to be
//! overridden every time, Kotlin's default is already what is wanted.
//!
//! ## Where the IDL spelling of a name is kept
//!
//! As in the Zig and Java columns: nothing is renamed for style, and a
//! rename is always the keyword escape, [`escape_identifier`]. The IDL
//! spelling goes in the comment above the renamed declaration, because
//! DDS-XTypes 1.3 §7.3.1.2.1.1 computes a member's `@autoid(HASH)` id
//! from the first four octets of the MD5 of its **IDL** name, and the
//! escape does not invert.

use crate::common::{BodyText, Doc, Ident, Literal, Path};
use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// A Kotlin source file.
///
/// One file per IDL file, as in every column with no `#include` graph
/// of its own. Kotlin's package system nests exactly the way an IDL
/// `module` does, so — unlike [`crate::zig`], [`crate::c`] and
/// [`crate::go`] — a nested module could in principle become a nested
/// package; it does not here, so that a probe compiling one file has one
/// name for every declaration and needs no second file per module. The
/// adapter's documentation carries the reason in full.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct File {
    /// The file's own leading comment.
    pub doc: Doc,
    /// The `package` line, absent for the default package.
    pub package: Option<Path>,
    /// `import` lines, in written order.
    pub imports: Vec<Path>,
    /// Top-level declarations, in written order.
    pub decls: Vec<Decl>,
}

/// One top-level declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decl {
    /// `data class Name(var a: T = x, ...)`.
    Class(Class),
    /// `object Name { const val A: T = x }` — an enumeration's
    /// constants, or a namespace of file-scope constants a bitmask's
    /// bits need.
    Object(Object),
    /// `typealias Name = T`.
    Alias(AliasDecl),
    /// `const val NAME: T = value`, or `val` where the value is not one
    /// `const val` accepts — an aggregate constant has no such form and
    /// is refused before this is reached.
    Const(ConstDecl),
    /// A free function — a generated codec routine, or a helper of the
    /// generator's own.
    Func(FuncDecl),
    /// `class Name(val op: (A) -> R, ...)` — an interface's operations
    /// as function-typed properties.
    Interface(Class),
    /// A comment standing on its own.
    Comment(Doc),
    /// `@verbatim` text, copied into the output unchanged — OMG IDL 4.2
    /// §8.3.5.1 asks for the text "verbatim", so raw lines and never a
    /// comment.
    Verbatim(String),
}

/// `data class Name(var a: T = x, ...)`, or `class` where `is_data` is
/// false — an interface's function-pointer holder is a plain `class`,
/// because `data class` requires at least one structural property and
/// gives every property `equals`/`hashCode`/`copy`, none of which a
/// bundle of behaviour needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Class {
    /// Documentation.
    pub doc: Doc,
    /// Whether this is a `data class`.
    pub is_data: bool,
    /// The name.
    pub name: Ident,
    /// The properties, in the primary constructor, in written order.
    pub properties: Vec<Property>,
    /// `@verbatim` text at the start and the end of the class body, which
    /// is written after the primary constructor only when there is text.
    pub body_text: BodyText,
}

/// One property of a class's primary constructor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Property {
    /// Documentation.
    pub doc: Doc,
    /// Whether it is `var` (Kotlin's default here) or `val` — every
    /// generated data class property is `var`, because a codec fills it
    /// in after construction; an interface's function-typed property is
    /// `val`, because nothing ever reassigns it.
    pub mutable: bool,
    /// The name.
    pub name: Ident,
    /// The type.
    pub type_: Type,
    /// The default value in the primary constructor. Present on every
    /// property of a generated data class, which is what lets
    /// `crate::emit::kotlin`'s `storage` call the class's own
    /// zero-argument constructor; absent on an interface's property,
    /// which the constructor call always supplies.
    pub default: Option<Expr>,
}

/// `object Name { ... }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Object {
    /// Documentation.
    pub doc: Doc,
    /// The name.
    pub name: Ident,
    /// The members, in written order.
    pub members: Vec<ConstDecl>,
}

/// `typealias Name = T`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AliasDecl {
    /// Documentation.
    pub doc: Doc,
    /// The name.
    pub name: Ident,
    /// The aliased type.
    pub target: Type,
}

/// `const val Name: T = value`, or `val` where the value cannot be a
/// Kotlin compile-time constant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstDecl {
    /// Documentation.
    pub doc: Doc,
    /// Whether the Kotlin compiler accepts this as `const val` — a
    /// primitive or `String` initialised by a constant expression
    /// (Kotlin language specification, "Const properties"). An
    /// aggregate-typed constant has no IDL form (§7.4.1.9 gives a
    /// constant only a primitive, `string`, `fixed` or enumeration
    /// type), so this is never false for a value this tree carries —
    /// the field exists so the renderer's choice of keyword is written
    /// down rather than assumed.
    pub is_const: bool,
    /// The name.
    pub name: Ident,
    /// The declared type.
    pub type_: Type,
    /// The value.
    pub value: Expr,
}

/// `fun name(a: T, b: U): R { body }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuncDecl {
    /// Documentation.
    pub doc: Doc,
    /// The name.
    pub name: Ident,
    /// Parameters, in order.
    pub params: Vec<Param>,
    /// The result type. `Unit` where nothing is returned, because Kotlin
    /// states it either way (Kotlin language specification, "Function
    /// declarations").
    pub returns: Type,
    /// The body.
    pub body: Vec<Stmt>,
}

/// One parameter of a function or an interface operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    /// The name.
    pub name: Ident,
    /// The type.
    pub type_: Type,
}

/// A Kotlin type, as it will be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    /// A built-in.
    Primitive(Primitive),
    /// A declared or runtime name, with any type arguments.
    Named {
        /// The path.
        path: Path,
        /// Type arguments, in written order.
        args: Vec<Type>,
    },
    /// `Array<T>`, or `TArray` where `T` is a primitive the JVM has a
    /// specialised array for — the renderer's choice, carried by
    /// `crate::render::kotlin` rather than by this tree, so that a
    /// `Sequence<Int?>` still reads `Array<Int?>` and not `IntArray`,
    /// which cannot hold a `null`.
    Array(Box<Type>),
    /// `T?`.
    Nullable(Box<Type>),
    /// `(A, B) -> R` — an interface operation's type.
    Function {
        /// Parameter types, in order.
        params: Vec<Type>,
        /// The result.
        returns: Box<Type>,
    },
    /// `Unit`.
    Unit,
}

impl Type {
    /// A type spelled by one unqualified name.
    #[must_use]
    pub fn named(text: &str) -> Self {
        Self::Named {
            path: Path::single(text),
            args: Vec::new(),
        }
    }

    /// A generic type with one argument: `List<T>`.
    #[must_use]
    pub fn generic1(text: &str, arg: Self) -> Self {
        Self::Named {
            path: Path::single(text),
            args: alloc::vec![arg],
        }
    }

    /// `Array<T>`.
    #[must_use]
    pub fn array(element: Self) -> Self {
        Self::Array(Box::new(element))
    }

    /// `T?`.
    #[must_use]
    pub fn nullable(inner: Self) -> Self {
        Self::Nullable(Box::new(inner))
    }
}

/// A Kotlin built-in type.
///
/// Kotlin's unsigned family — `UByte`, `UShort`, `UInt`, `ULong` — is
/// deliberately absent. It is a library type over the signed one with a
/// narrower standard library (no `Random.nextUByte`, no unsigned range
/// literal), it is `@ExperimentalUnsignedTypes` outside a handful of
/// stabilised operators as of Kotlin 2.0, and the sibling Java column
/// already answers "how does a JVM target without unsigned arithmetic
/// hold a `uint32`" — the next signed width up. Reusing that answer
/// keeps the two JVM columns disagreeing about nothing but syntax.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Primitive {
    /// `Boolean`
    Boolean,
    /// `Byte`
    Byte,
    /// `Short`
    Short,
    /// `Int`
    Int,
    /// `Long`
    Long,
    /// `Float`
    Float,
    /// `Double`
    Double,
    /// `Char`
    Char,
}

impl Primitive {
    /// Every primitive this tree can hold.
    pub const ALL: [Self; 8] = [
        Self::Boolean,
        Self::Byte,
        Self::Short,
        Self::Int,
        Self::Long,
        Self::Float,
        Self::Double,
        Self::Char,
    ];

    /// How it is written.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Boolean => "Boolean",
            Self::Byte => "Byte",
            Self::Short => "Short",
            Self::Int => "Int",
            Self::Long => "Long",
            Self::Float => "Float",
            Self::Double => "Double",
            Self::Char => "Char",
        }
    }
}

/// A block of statements.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Block {
    /// The statements, in order.
    pub statements: Vec<Stmt>,
}

/// A statement, as the codec walk builds one.
///
/// Only the forms the shared walk emits and the two loop shapes it
/// needs — the module documentation for [`crate::zig`] and
/// [`crate::java`] gives the same restriction for the same reason: a
/// form no walk builds is a renderer arm no test reaches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    /// `val name: T = value` or `var name: T = value`.
    Declare {
        /// Whether it is `var`.
        mutable: bool,
        /// The type. Written out even where inference would find it,
        /// because a generated file is read far more often than it is
        /// written and the width of a wire-facing local is exactly what
        /// a reader wants in front of them.
        type_: Type,
        /// The name.
        name: Ident,
        /// The initialiser.
        value: Option<Expr>,
    },
    /// An expression evaluated for its effect.
    Expr(Expr),
    /// `return value`, or a bare `return`.
    Return(Option<Expr>),
    /// `if (cond) { .. } else { .. }`.
    If {
        /// The condition.
        condition: Expr,
        /// The then-branch.
        then: Block,
        /// The else-branch, absent where there is none.
        otherwise: Option<Block>,
    },
    /// `for (name in 0 until bound) { .. }`.
    CountUp {
        /// The counter's name.
        counter: Ident,
        /// The exclusive bound.
        bound: Box<Expr>,
        /// The body.
        body: Block,
    },
    /// `while (cond) { .. }`.
    While {
        /// The condition.
        condition: Expr,
        /// The body.
        body: Block,
    },
    /// `break`.
    Break,
    /// `throw value`.
    Throw(Expr),
    /// A comment on its own line.
    Comment(Doc),
}

/// An expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    /// A literal.
    Literal(Literal),
    /// A name or a qualified path.
    Path(Path),
    /// `receiver.name(args)`, or `name(args)` where `receiver` is
    /// absent.
    Call {
        /// The receiver.
        receiver: Option<Box<Expr>>,
        /// The name called.
        name: Ident,
        /// Arguments, in order.
        args: Vec<Expr>,
    },
    /// `value.name`.
    Field {
        /// The value.
        value: Box<Expr>,
        /// The field.
        name: Ident,
    },
    /// `a[i]`.
    Index {
        /// The array.
        value: Box<Expr>,
        /// The index.
        index: Box<Expr>,
    },
    /// `Type(args)` — a constructor call. Kotlin writes no `new`
    /// (Kotlin language specification, "Object creation expressions").
    New {
        /// The type constructed.
        type_: Box<Type>,
        /// Constructor arguments, in order.
        args: Vec<Expr>,
    },
    /// `IntArray(n)`, or `Array(n) { T() }` for a reference element —
    /// room for `n` values, every slot holding a real instance from the
    /// moment it is allocated. `Array(size, init)` is what makes the
    /// second form possible without ever routing a `null` through a
    /// non-nullable element type (Kotlin language specification, "Array
    /// types"): unlike `new T[n]` in Java, there is no all-null state to
    /// leave a `map`'s entry halves unset in before the fill runs, so
    /// `crate::emit::kotlin`'s `make_room` needs no separate
    /// pre-construction loop for either the sequence or the `map` case.
    NewArray {
        /// The element type.
        element: Box<Type>,
        /// The length.
        length: Box<Expr>,
    },
    /// `arrayOf(a, b, c)`, or the primitive-specialised
    /// `intArrayOf`/`byteArrayOf`/… the renderer picks for a primitive
    /// element — the literal form, for the write direction's sample.
    ArrayOf {
        /// The element type, which decides the specialised factory.
        element: Box<Type>,
        /// The elements, in order.
        elements: Vec<Expr>,
    },
    /// `a op b`.
    Binary {
        /// The operator, as written.
        op: &'static str,
        /// The left operand.
        lhs: Box<Expr>,
        /// The right operand.
        rhs: Box<Expr>,
    },
    /// `(value)`, written because the caller meant it — the tree carries
    /// no precedence.
    Paren(Box<Expr>),
    /// `value as T` — Kotlin's unchecked cast.
    As {
        /// The value.
        value: Box<Expr>,
        /// The target type.
        type_: Box<Type>,
    },
    /// `cond.then(a).otherwise(b)`, written `if (cond) a else b`: Kotlin
    /// has no separate ternary operator and its `if` is already an
    /// expression (Kotlin language specification, "Conditional
    /// expressions").
    If {
        /// The condition.
        condition: Box<Expr>,
        /// The value where it holds.
        then: Box<Expr>,
        /// The value where it does not.
        otherwise: Box<Expr>,
    },
    /// `!value` — boolean negation. The codec walk's only prefix
    /// operator (`crates/lang-rules/src/emit/mod.rs`'s shared walk names
    /// `!` and `~`, and `~` has no prefix spelling in Kotlin — see
    /// `crate::emit::kotlin`, which reaches [`Primitive`]'s `inv()`
    /// instead).
    Not(Box<Expr>),
    /// `value!!` — Kotlin's not-null assertion (Kotlin language
    /// specification, "Not-null assertion expressions"). Reached where
    /// the codec walk has already tested presence and reads an
    /// `@optional` member's value back: a `var` property's type stays
    /// `T?` at the read, because Kotlin only smart-casts a value that
    /// cannot change between the test and the use, and a mutable
    /// property is exactly the one the compiler will not trust
    /// (measured: `if (v.x != null) { foo(v.x) }` on a `var x: T?`
    /// refuses with "Smart cast... impossible, because 'x' is a var").
    NotNull(Box<Expr>),
}

impl Expr {
    /// A bare name.
    #[must_use]
    pub fn name(text: &str) -> Self {
        Self::Path(Path::single(text))
    }

    /// An integer literal.
    #[must_use]
    pub fn int(value: i128) -> Self {
        Self::Literal(Literal::int(value))
    }

    /// `value.name`.
    #[must_use]
    pub fn field(value: Self, name: &str) -> Self {
        Self::Field {
            value: Box::new(value),
            name: Ident::new(name),
        }
    }

    /// `name(args)`.
    #[must_use]
    pub fn call(name: &str, args: Vec<Self>) -> Self {
        Self::Call {
            receiver: None,
            name: Ident::new(name),
            args,
        }
    }

    /// `receiver.name(args)`.
    #[must_use]
    pub fn method_call(receiver: Self, name: &str, args: Vec<Self>) -> Self {
        Self::Call {
            receiver: Some(Box::new(receiver)),
            name: Ident::new(name),
            args,
        }
    }
}

/// The name a Kotlin hard keyword collision escapes to.
///
/// A trailing underscore, as in the Zig and Java columns. Kotlin also
/// has an escape of its own — `` `class` `` in backticks accepts any
/// text between the backticks, including a soft keyword or a string with
/// spaces (Kotlin language specification, "Identifiers") — and it is not
/// used here for the same two reasons [`crate::zig`]'s module
/// documentation gives for not using `@"…"`: it would accept spellings
/// no IDL identifier can produce, so the escape would stop being visible
/// as one, and a backtick-quoted name reads as an escape hatch for
/// broken code rather than as an ordinary field in a generated file next
/// to `class_` and `object_`.
#[must_use]
pub fn escape_identifier(name: &str) -> String {
    if HARD_KEYWORDS.contains(&name) {
        let mut out = name.to_string();
        out.push('_');
        return out;
    }
    name.to_string()
}

/// Kotlin's hard keywords — reserved in every position and never usable
/// as a plain identifier without backticks (Kotlin language
/// specification, "Keywords and operators", "Hard keywords").
///
/// The *soft* and *modifier* keywords (`data`, `object`, `import`,
/// `field`, …) are not here: each parses as an identifier everywhere but
/// where the grammar specifically expects the keyword, so
/// `struct Data { data: Int }`'s `data` member needs no escape — measured
/// against Kotlin 2.0's grammar, which lists them in a table separate
/// from the hard keywords precisely because of that difference.
pub const HARD_KEYWORDS: &[&str] = &[
    "as",
    "break",
    "class",
    "continue",
    "do",
    "else",
    "false",
    "for",
    "fun",
    "if",
    "in",
    "interface",
    "is",
    "null",
    "object",
    "package",
    "return",
    "super",
    "this",
    "throw",
    "true",
    "try",
    "typealias",
    "typeof",
    "val",
    "var",
    "when",
    "while",
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
    fn no_unsigned_primitive_exists() {
        // The module documentation says why: Kotlin's unsigned family is
        // a library type with a narrower standard library and is still
        // partly experimental, and the Java column already answers the
        // question of a JVM target with no unsigned arithmetic.
        for primitive in Primitive::ALL {
            assert!(!primitive.as_str().starts_with('U'), "{primitive:?}");
        }
    }

    #[test]
    fn a_hard_keyword_escapes_with_a_trailing_underscore() {
        for name in ["class", "object", "val", "fun", "when"] {
            assert_eq!(escape_identifier(name), alloc::format!("{name}_"));
        }
    }

    #[test]
    fn a_soft_keyword_is_not_escaped() {
        // `data`, `field` and `import` parse as identifiers everywhere
        // but where the grammar specifically wants the keyword.
        for name in ["data", "field", "import", "value", "constructor"] {
            assert_eq!(escape_identifier(name), name);
        }
    }

    #[test]
    fn a_plain_name_is_left_exactly_as_the_idl_wrote_it() {
        for name in ["Pose", "pose", "x_min", "_d", "MODULE_ID"] {
            assert_eq!(escape_identifier(name), name);
        }
    }

    #[test]
    fn hard_keywords_do_not_repeat() {
        let mut seen: Vec<&str> = Vec::new();
        for name in HARD_KEYWORDS {
            assert!(!seen.contains(name), "{name} is listed twice");
            seen.push(name);
        }
    }

    #[test]
    fn a_property_can_carry_the_default_that_makes_the_zero_argument_constructor_work() {
        let property = Property {
            doc: Doc::none(),
            mutable: true,
            name: Ident::new("x"),
            type_: Type::Primitive(Primitive::Int),
            default: Some(Expr::int(0)),
        };
        assert_eq!(property.default, Some(Expr::int(0)));
    }

    #[test]
    fn an_array_of_a_primitive_and_of_a_named_type_are_both_expressible() {
        let ints = Type::array(Type::Primitive(Primitive::Int));
        let Type::Array(element) = &ints else {
            panic!("not an array");
        };
        assert_eq!(**element, Type::Primitive(Primitive::Int));
        let names = Type::array(Type::named("Pose"));
        assert_eq!(names, Type::Array(Box::new(Type::named("Pose"))));
    }
}
