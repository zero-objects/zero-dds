// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The completeness table. Spec §4.1.
//!
//! One cell per construct and language, each stating the rule that
//! covers it or that it is deliberately uncovered, with a reason.
//!
//! The table has no catch-all arm. A construct added to the common AST
//! without a decision here is a compile error, not a language backend
//! that silently emits nothing for it.

use zerodds_lang_cst::Language;

/// The constructs a rule catalogue has to account for.
///
/// The same 21 `zerodds-ast-1.0` §8.2 rules on, so the two tables can be
/// read against each other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ConstructKind {
    /// A module.
    Module,
    /// A primitive type.
    Primitive,
    /// `fixed<d,s>`.
    Fixed,
    /// `string` and `wstring`.
    StringType,
    /// `sequence<T>`.
    Sequence,
    /// An array.
    Array,
    /// `map<K,V>`.
    Map,
    /// A struct.
    Structure,
    /// A union.
    Union,
    /// An enumeration.
    Enumeration,
    /// A bitmask.
    Bitmask,
    /// A bitset.
    Bitset,
    /// A typedef.
    Alias,
    /// A valuetype.
    Valuetype,
    /// A boxed value.
    ValueBox,
    /// A native type.
    Native,
    /// `any`.
    Any,
    /// `ValueBase`.
    ValueBase,
    /// An object reference.
    ObjectRef,
    /// An annotation type.
    AnnotationType,
    /// A constant.
    Const,
    /// An interface.
    Interface,
    /// An exception.
    Exception,
}

impl ConstructKind {
    /// Every construct the table covers.
    pub const ALL: [Self; 23] = [
        Self::Module,
        Self::Primitive,
        Self::Fixed,
        Self::StringType,
        Self::Sequence,
        Self::Array,
        Self::Map,
        Self::Structure,
        Self::Union,
        Self::Enumeration,
        Self::Bitmask,
        Self::Bitset,
        Self::Alias,
        Self::Valuetype,
        Self::ValueBox,
        Self::Native,
        Self::Any,
        Self::ValueBase,
        Self::ObjectRef,
        Self::AnnotationType,
        Self::Const,
        Self::Interface,
        Self::Exception,
    ];

    /// The name a table column uses.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Module => "module",
            Self::Primitive => "primitive",
            Self::Fixed => "fixed",
            Self::StringType => "string",
            Self::Sequence => "sequence",
            Self::Array => "array",
            Self::Map => "map",
            Self::Structure => "struct",
            Self::Union => "union",
            Self::Enumeration => "enum",
            Self::Bitmask => "bitmask",
            Self::Bitset => "bitset",
            Self::Alias => "typedef",
            Self::Valuetype => "valuetype",
            Self::ValueBox => "valuebox",
            Self::Native => "native",
            Self::Any => "any",
            Self::ValueBase => "valuebase",
            Self::ObjectRef => "object reference",
            Self::AnnotationType => "annotation type",
            Self::Const => "const",
            Self::Interface => "interface",
            Self::Exception => "exception",
        }
    }
}

/// What a cell of the table says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Coverage {
    /// A rule covers it, named here.
    Rule(&'static str),
    /// Deliberately uncovered, with the reason.
    Uncovered(&'static str),
}

impl Coverage {
    /// Whether a rule covers this cell.
    #[must_use]
    pub const fn is_covered(self) -> bool {
        matches!(self, Self::Rule(_))
    }

    /// The rule's name, or the reason it has none.
    #[must_use]
    pub const fn reason(self) -> &'static str {
        match self {
            Self::Rule(name) | Self::Uncovered(name) => name,
        }
    }
}

/// What covers `construct` in `language`.
///
/// Exhaustive over both, with no catch-all: adding a construct or a
/// language is a compile error here until a decision is written down.
#[must_use]
#[allow(clippy::too_many_lines)]
pub const fn coverage_of(construct: ConstructKind, language: Language) -> Coverage {
    use ConstructKind as C;
    use Language as L;

    match (construct, language) {
        // -- Rust: every construct has a rule ---------------------------
        (C::Structure, L::Rust) => Coverage::Rule("struct-to-struct"),
        (C::Enumeration, L::Rust) => Coverage::Rule("enum-to-enum"),
        (C::Alias, L::Rust) => Coverage::Rule("alias-to-type-alias"),
        (C::Union, L::Rust) => Coverage::Rule("union-to-enum"),
        (C::Bitmask, L::Rust) => Coverage::Rule("bitmask-to-struct"),
        (C::Bitset, L::Rust) => Coverage::Rule("bitset-to-struct"),
        (C::Valuetype, L::Rust) => Coverage::Rule("valuetype-to-struct"),
        (C::Module, L::Rust) => Coverage::Rule("module-to-mod"),
        (C::Const, L::Rust) => Coverage::Rule("const-to-const"),
        (C::Exception, L::Rust) => Coverage::Rule("exception-to-struct"),
        (C::Interface, L::Rust) => Coverage::Rule("interface-to-trait"),
        (C::Native, L::Rust) => Coverage::Rule("native-to-alias"),
        // The four that are not declarations of their own: a member's
        // type becomes a Rust type where the member is materialised,
        // which is the `member-to-field` rule's doing.
        (C::Primitive | C::Sequence | C::Array | C::StringType | C::Map | C::Fixed, L::Rust) => {
            Coverage::Rule("member-to-field")
        }
        (C::ValueBox, L::Rust) => Coverage::Rule("member-to-field"),
        (C::ObjectRef, L::Rust) => Coverage::Uncovered("object references belong to the RPC arc"),
        (C::Any, L::Rust) => {
            Coverage::Uncovered("`any` carries its type at run time; the CORBA arc owns it")
        }
        (C::ValueBase, L::Rust) => Coverage::Uncovered(
            "which valuetype arrives is a run-time question; the CORBA arc owns it",
        ),
        (C::AnnotationType, L::Rust) => {
            Coverage::Uncovered("no Rust equivalent; nothing is emitted")
        }

        // -- C: the second column, and the pattern the rest follow ------
        //
        // A module is covered here where it is uncovered in C++ and
        // Java: C has no namespaces, so `module-to-prefix` is not a
        // placeholder for a scope rule — flattening the scoped name onto
        // every identifier *is* what a module becomes.
        (C::Module, L::C) => Coverage::Rule("module-to-prefix"),
        (C::Structure, L::C) => Coverage::Rule("struct-to-struct"),
        (C::Enumeration, L::C) => Coverage::Rule("enum-to-enum"),
        (C::Alias, L::C) => Coverage::Rule("alias-to-typedef"),
        (C::Union, L::C) => Coverage::Rule("union-to-union"),
        (C::Bitmask, L::C) => Coverage::Rule("bitmask-to-enum"),
        (C::Bitset, L::C) => Coverage::Rule("bitset-to-struct"),
        (C::Valuetype, L::C) => Coverage::Rule("valuetype-to-struct"),
        (C::Const, L::C) => Coverage::Rule("const-to-const"),
        (C::Exception, L::C) => Coverage::Rule("exception-to-struct"),
        (C::Native, L::C) => Coverage::Rule("native-to-typedef"),
        // The six that are not declarations of their own. C has no
        // generics, so each becomes a declared support type where the
        // member is materialised — which is still `member-to-member`'s
        // doing, the same way `member-to-field` owns them in Rust.
        (
            C::Primitive | C::Sequence | C::Array | C::StringType | C::Map | C::Fixed | C::ValueBox,
            L::C,
        ) => Coverage::Rule("member-to-member"),
        (C::ObjectRef, L::C) => Coverage::Uncovered("object references belong to the RPC arc"),
        (C::Any, L::C) => {
            Coverage::Uncovered("`any` carries its type at run time; the CORBA arc owns it")
        }
        (C::ValueBase, L::C) => Coverage::Uncovered(
            "which valuetype arrives is a run-time question; the CORBA arc owns it",
        ),
        (C::AnnotationType, L::C) => Coverage::Uncovered("no C equivalent; nothing is emitted"),
        // Uncovered and not merely unwritten: a C interface is a struct
        // of function pointers, and `zerodds_lang_cst::c::Type` has no
        // function-pointer form — the name sits inside the declarator,
        // which the tree cannot express. Adding the rule before the tree
        // can hold its output would be a catalogue claiming coverage
        // that materialises to nothing.
        (C::Interface, L::C) => {
            Coverage::Uncovered("the C CST has no function-pointer type to declare one with")
        }

        // -- the two catalogues that follow ------------------------------
        //
        // Named rather than left out: a table that listed only what is
        // done would read as a table that is done.
        (C::Module, L::Cpp) => Coverage::Uncovered("module-to-namespace, not yet written"),
        (C::Module, L::Java) => Coverage::Uncovered("module-to-package, not yet written"),
        (
            C::Primitive
            | C::Fixed
            | C::StringType
            | C::Sequence
            | C::Array
            | C::Map
            | C::Structure
            | C::Union
            | C::Enumeration
            | C::Bitmask
            | C::Bitset
            | C::Alias
            | C::Valuetype
            | C::ValueBox
            | C::Native
            | C::Any
            | C::ValueBase
            | C::ObjectRef
            | C::AnnotationType
            | C::Const
            | C::Interface
            | C::Exception,
            L::Cpp | L::Java,
        ) => Coverage::Uncovered("catalogue not yet written for this language"),
        // -- C#: every construct that has a data form ---------------------
        (C::Structure, L::CSharp) => Coverage::Rule("struct-to-class"),
        (C::Enumeration, L::CSharp) => Coverage::Rule("enum-to-enum"),
        (C::Alias, L::CSharp) => Coverage::Rule("alias-to-using"),
        (C::Union, L::CSharp) => Coverage::Rule("union-to-class"),
        (C::Bitmask, L::CSharp) => Coverage::Rule("bitmask-to-flags-enum"),
        (C::Bitset, L::CSharp) => Coverage::Rule("bitset-to-class"),
        (C::Valuetype, L::CSharp) => Coverage::Rule("valuetype-to-class"),
        (C::Module, L::CSharp) => Coverage::Rule("module-to-namespace"),
        (C::Const, L::CSharp) => Coverage::Rule("const-to-const"),
        (C::Exception, L::CSharp) => Coverage::Rule("exception-to-class"),
        (C::Interface, L::CSharp) => Coverage::Rule("interface-to-interface"),
        (C::Native, L::CSharp) => Coverage::Rule("native-to-alias"),
        (C::Primitive, L::CSharp) => Coverage::Rule("member-to-field"),
        (C::StringType, L::CSharp) => Coverage::Rule("member-to-field"),
        (C::Sequence, L::CSharp) => Coverage::Rule("member-to-field"),
        (C::Array, L::CSharp) => Coverage::Rule("member-to-field"),
        (C::Map, L::CSharp) => Coverage::Rule("member-to-field"),
        (C::Fixed, L::CSharp) => Coverage::Rule("member-to-field"),
        (C::ValueBox, L::CSharp) => Coverage::Rule("member-to-field"),
        (C::Any, L::CSharp) => Coverage::Uncovered(
            "an `any` needs the dynamic type system, which is the reflective arc",
        ),
        (C::ValueBase, L::CSharp) => {
            Coverage::Uncovered("no C# form; a valuetype base is not a declaration of its own")
        }
        (C::ObjectRef, L::CSharp) => Coverage::Uncovered("object references belong to the RPC arc"),
        (C::AnnotationType, L::CSharp) => Coverage::Uncovered("an annotation declares no data"),

        // -- Python -------------------------------------------------------
        (C::Structure, L::Python) => Coverage::Rule("struct-to-dataclass"),
        (C::Enumeration, L::Python) => Coverage::Rule("enum-to-intenum"),
        (C::Alias, L::Python) => Coverage::Rule("alias-to-comment"),
        (C::Union, L::Python) => Coverage::Rule("union-to-dataclass"),
        (C::Bitmask, L::Python) => Coverage::Rule("bitmask-to-intflag"),
        (C::Bitset, L::Python) => Coverage::Rule("bitset-to-dataclass"),
        (C::Valuetype, L::Python) => Coverage::Rule("valuetype-to-dataclass"),
        (C::Module, L::Python) => Coverage::Rule("module-to-class"),
        (C::Const, L::Python) => Coverage::Rule("const-to-final"),
        (C::Exception, L::Python) => Coverage::Rule("exception-to-dataclass"),
        (C::Interface, L::Python) => Coverage::Uncovered(
            "the rule fires and materialises nothing; an IDL interface has no data form in a types-only column",
        ),
        (C::Native, L::Python) => Coverage::Rule("native-to-comment"),
        (C::Primitive, L::Python) => Coverage::Rule("member-to-field"),
        (C::StringType, L::Python) => Coverage::Rule("member-to-field"),
        (C::Sequence, L::Python) => Coverage::Rule("member-to-field"),
        (C::Array, L::Python) => Coverage::Rule("member-to-field"),
        (C::Map, L::Python) => Coverage::Rule("member-to-field"),
        (C::Fixed, L::Python) => Coverage::Rule("member-to-field"),
        (C::ValueBox, L::Python) => Coverage::Rule("member-to-field"),
        (C::Any, L::Python) => Coverage::Uncovered(
            "an `any` needs the dynamic type system, which is the reflective arc",
        ),
        (C::ValueBase, L::Python) => {
            Coverage::Uncovered("no Python form; a valuetype base is not a declaration of its own")
        }
        (C::ObjectRef, L::Python) => Coverage::Uncovered("object references belong to the RPC arc"),
        (C::AnnotationType, L::Python) => Coverage::Uncovered("an annotation declares no data"),

        // -- TypeScript ---------------------------------------------------
        (C::Structure, L::TypeScript) => Coverage::Rule("struct-to-interface"),
        (C::Enumeration, L::TypeScript) => Coverage::Rule("enum-to-const-object"),
        (C::Alias, L::TypeScript) => Coverage::Rule("alias-to-type-alias"),
        (C::Union, L::TypeScript) => Coverage::Rule("union-to-discriminated-union"),
        (C::Bitmask, L::TypeScript) => Coverage::Rule("bitmask-to-const-object"),
        (C::Bitset, L::TypeScript) => Coverage::Rule("bitset-to-interface"),
        (C::Valuetype, L::TypeScript) => Coverage::Rule("valuetype-to-interface"),
        (C::Module, L::TypeScript) => Coverage::Rule("module-to-namespace"),
        (C::Const, L::TypeScript) => Coverage::Rule("const-to-const"),
        (C::Exception, L::TypeScript) => Coverage::Rule("exception-to-interface"),
        (C::Interface, L::TypeScript) => Coverage::Rule("interface-to-interface"),
        (C::Native, L::TypeScript) => Coverage::Rule("native-to-type-alias"),
        (C::Primitive, L::TypeScript) => Coverage::Rule("member-to-property"),
        (C::StringType, L::TypeScript) => Coverage::Rule("member-to-property"),
        (C::Sequence, L::TypeScript) => Coverage::Rule("member-to-property"),
        (C::Array, L::TypeScript) => Coverage::Rule("member-to-property"),
        (C::Map, L::TypeScript) => Coverage::Rule("member-to-property"),
        (C::Fixed, L::TypeScript) => Coverage::Rule("member-to-property"),
        (C::ValueBox, L::TypeScript) => Coverage::Rule("member-to-property"),
        (C::Any, L::TypeScript) => Coverage::Uncovered(
            "an `any` needs the dynamic type system, which is the reflective arc",
        ),
        (C::ValueBase, L::TypeScript) => Coverage::Uncovered(
            "no TypeScript form; a valuetype base is not a declaration of its own",
        ),
        (C::ObjectRef, L::TypeScript) => {
            Coverage::Uncovered("object references belong to the RPC arc")
        }
        (C::AnnotationType, L::TypeScript) => Coverage::Uncovered("an annotation declares no data"),

        // -- Go -----------------------------------------------------------
        //
        // A module is covered here as it is in C and for the same
        // reason: a Go package is a directory, so nested IDL modules
        // cannot nest inside one generated file, and flattening the
        // scoped name onto every identifier *is* what a module becomes.
        // An interface is covered here and not in C, because a Go
        // interface is a method set with names and the C tree has no
        // function-pointer type to write one with.
        (C::Module, L::Go) => Coverage::Rule("module-to-prefix"),
        (C::Structure, L::Go) => Coverage::Rule("struct-to-struct"),
        (C::Enumeration, L::Go) => Coverage::Rule("enum-to-type"),
        (C::Alias, L::Go) => Coverage::Rule("alias-to-alias"),
        (C::Union, L::Go) => Coverage::Rule("union-to-struct"),
        (C::Bitmask, L::Go) => Coverage::Rule("bitmask-to-type"),
        (C::Bitset, L::Go) => Coverage::Rule("bitset-to-struct"),
        (C::Valuetype, L::Go) => Coverage::Rule("valuetype-to-struct"),
        (C::Const, L::Go) => Coverage::Rule("const-to-const"),
        (C::Exception, L::Go) => Coverage::Rule("exception-to-struct"),
        (C::Native, L::Go) => Coverage::Rule("native-to-alias"),
        (C::Interface, L::Go) => Coverage::Rule("interface-to-interface"),
        // The seven that are not declarations of their own. Go writes
        // `[]T`, `[N]T` and `string` where the member is, so each
        // becomes a type at the point of use — which is
        // `member-to-field`'s doing, the same way it owns them in Rust.
        (
            C::Primitive | C::Sequence | C::Array | C::StringType | C::Map | C::Fixed | C::ValueBox,
            L::Go,
        ) => Coverage::Rule("member-to-field"),
        (C::ObjectRef, L::Go) => Coverage::Uncovered("object references belong to the RPC arc"),
        (C::Any, L::Go) => Coverage::Uncovered(
            "`any` carries its type at run time; Go's own `any` would compile and \
             there would be no wire form behind it",
        ),
        (C::ValueBase, L::Go) => Coverage::Uncovered(
            "which valuetype arrives is a run-time question; the CORBA arc owns it",
        ),
        (C::AnnotationType, L::Go) => Coverage::Uncovered("an annotation declares no data"),

        // -- OCaml --------------------------------------------------------
        //
        // A module flattens as it does in C and Go, and for a reason
        // neither of those has: OCaml resolves a cycle with an
        // `and`-joined group, and such a group cannot cross a
        // `module … struct … end` boundary, while IDL puts no
        // restriction on where a cycle closes.
        (C::Module, L::OCaml) => Coverage::Rule("module-to-prefix"),
        (C::Structure, L::OCaml) => Coverage::Rule("struct-to-record"),
        (C::Enumeration, L::OCaml) => Coverage::Rule("enum-to-variant"),
        (C::Alias, L::OCaml) => Coverage::Rule("alias-to-abbreviation"),
        (C::Union, L::OCaml) => Coverage::Rule("union-to-record"),
        (C::Bitmask, L::OCaml) => Coverage::Rule("bitmask-to-abbreviation"),
        (C::Bitset, L::OCaml) => Coverage::Rule("bitset-to-record"),
        (C::Valuetype, L::OCaml) => Coverage::Rule("valuetype-to-record"),
        (C::Const, L::OCaml) => Coverage::Rule("const-to-let"),
        (C::Exception, L::OCaml) => Coverage::Rule("exception-to-record"),
        (C::Native, L::OCaml) => Coverage::Rule("native-to-abbreviation"),
        (C::Interface, L::OCaml) => Coverage::Rule("interface-to-class-type"),
        // The seven that are not declarations of their own: an OCaml
        // `array`, `option` and `string` are written where the member is.
        (
            C::Primitive | C::Sequence | C::Array | C::StringType | C::Map | C::Fixed | C::ValueBox,
            L::OCaml,
        ) => Coverage::Rule("member-to-field"),
        (C::ObjectRef, L::OCaml) => Coverage::Uncovered("object references belong to the RPC arc"),
        (C::Any, L::OCaml) => {
            Coverage::Uncovered("`any` carries its type at run time; the CORBA arc owns it")
        }
        (C::ValueBase, L::OCaml) => Coverage::Uncovered(
            "which valuetype arrives is a run-time question; the CORBA arc owns it",
        ),
        (C::AnnotationType, L::OCaml) => Coverage::Uncovered("an annotation declares no data"),

        // -- Ada ---------------------------------------------------------
        //
        // A module flattens with `_` and a package would nest: GNAT ties a
        // compilation unit to a file name, so one generated spec is one
        // package.
        (C::Module, L::Ada) => Coverage::Rule("module-to-prefix"),
        (C::Structure, L::Ada) => Coverage::Rule("struct-to-record"),
        (C::Enumeration, L::Ada) => Coverage::Rule("enum-to-type"),
        (C::Alias, L::Ada) => Coverage::Rule("alias-to-subtype"),
        (C::Union, L::Ada) => Coverage::Rule("union-to-record"),
        (C::Bitmask, L::Ada) => Coverage::Rule("bitmask-to-type"),
        (C::Bitset, L::Ada) => Coverage::Rule("bitset-to-record"),
        (C::Valuetype, L::Ada) => Coverage::Rule("valuetype-to-record"),
        (C::Const, L::Ada) => Coverage::Rule("const-to-constant"),
        (C::Exception, L::Ada) => Coverage::Rule("exception-to-record"),
        (C::Native, L::Ada) => Coverage::Rule("native-to-type"),
        (C::Interface, L::Ada) => Coverage::Rule("interface-to-interface"),
        // The seven that are not declarations of their own: each becomes
        // a type at the point of use, which is the member rule's doing.
        (
            C::Primitive | C::Sequence | C::Array | C::StringType | C::Map | C::Fixed | C::ValueBox,
            L::Ada,
        ) => Coverage::Rule("member-to-component"),
        (C::ObjectRef, L::Ada) => Coverage::Uncovered("object references belong to the RPC arc"),
        (C::Any, L::Ada) => {
            Coverage::Uncovered("`any` carries its type at run time; the CORBA arc owns it")
        }
        (C::ValueBase, L::Ada) => Coverage::Uncovered(
            "which valuetype arrives is a run-time question; the CORBA arc owns it",
        ),
        (C::AnnotationType, L::Ada) => Coverage::Uncovered("an annotation declares no data"),

        // -- D -----------------------------------------------------------
        //
        // A module flattens with `_`: D has real modules and one module is one
        // file, so nested IDL modules cannot each be a D module here.
        (C::Module, L::D) => Coverage::Rule("module-to-prefix"),
        (C::Structure, L::D) => Coverage::Rule("struct-to-struct"),
        (C::Enumeration, L::D) => Coverage::Rule("enum-to-enum"),
        (C::Alias, L::D) => Coverage::Rule("alias-to-alias"),
        (C::Union, L::D) => Coverage::Rule("union-to-struct"),
        (C::Bitmask, L::D) => Coverage::Rule("bitmask-to-alias"),
        (C::Bitset, L::D) => Coverage::Rule("bitset-to-struct"),
        (C::Valuetype, L::D) => Coverage::Rule("valuetype-to-struct"),
        (C::Const, L::D) => Coverage::Rule("const-to-const"),
        (C::Exception, L::D) => Coverage::Rule("exception-to-struct"),
        (C::Native, L::D) => Coverage::Rule("native-to-alias"),
        (C::Interface, L::D) => Coverage::Rule("interface-to-interface"),
        // The seven that are not declarations of their own: each becomes
        // a type at the point of use, which is the member rule's doing.
        (
            C::Primitive | C::Sequence | C::Array | C::StringType | C::Map | C::Fixed | C::ValueBox,
            L::D,
        ) => Coverage::Rule("member-to-field"),
        (C::ObjectRef, L::D) => Coverage::Uncovered("object references belong to the RPC arc"),
        (C::Any, L::D) => Coverage::Uncovered(
            "`any` carries its type at run time; this language has a type that would \
             hold it and no wire form behind it",
        ),
        (C::ValueBase, L::D) => Coverage::Uncovered(
            "which valuetype arrives is a run-time question; the CORBA arc owns it",
        ),
        (C::AnnotationType, L::D) => Coverage::Uncovered("an annotation declares no data"),

        // -- Elixir ------------------------------------------------------
        //
        // A module **nests**, which no other flattening column can do: an Elixir
        // module name is a dotted atom and `Common.Util.Location` is a
        // genuine scope, not a flattened name.
        (C::Module, L::Elixir) => Coverage::Rule("module-to-module"),
        (C::Structure, L::Elixir) => Coverage::Rule("struct-to-module"),
        (C::Enumeration, L::Elixir) => Coverage::Rule("enum-to-module"),
        (C::Alias, L::Elixir) => Coverage::Rule("alias-to-type"),
        (C::Union, L::Elixir) => Coverage::Rule("union-to-module"),
        (C::Bitmask, L::Elixir) => Coverage::Rule("bitmask-to-module"),
        (C::Bitset, L::Elixir) => Coverage::Rule("bitset-to-module"),
        (C::Valuetype, L::Elixir) => Coverage::Rule("valuetype-to-module"),
        (C::Const, L::Elixir) => Coverage::Rule("const-to-function"),
        (C::Exception, L::Elixir) => Coverage::Rule("exception-to-module"),
        (C::Native, L::Elixir) => Coverage::Rule("native-to-type"),
        (C::Interface, L::Elixir) => Coverage::Rule("interface-to-behaviour"),
        // The seven that are not declarations of their own: each becomes
        // a type at the point of use, which is the member rule's doing.
        (
            C::Primitive | C::Sequence | C::Array | C::StringType | C::Map | C::Fixed | C::ValueBox,
            L::Elixir,
        ) => Coverage::Rule("member-to-field"),
        (C::ObjectRef, L::Elixir) => Coverage::Uncovered("object references belong to the RPC arc"),
        (C::Any, L::Elixir) => Coverage::Uncovered(
            "`any` carries its type at run time; this language has a type that would \
             hold it and no wire form behind it",
        ),
        (C::ValueBase, L::Elixir) => Coverage::Uncovered(
            "which valuetype arrives is a run-time question; the CORBA arc owns it",
        ),
        (C::AnnotationType, L::Elixir) => Coverage::Uncovered("an annotation declares no data"),

        // -- Julia -------------------------------------------------------
        //
        // A module flattens with `_`: Julia resolves a name in a module in
        // declaration order, and a nested module would put a forward
        // reference beyond reach.
        (C::Module, L::Julia) => Coverage::Rule("module-to-prefix"),
        (C::Structure, L::Julia) => Coverage::Rule("struct-to-struct"),
        (C::Enumeration, L::Julia) => Coverage::Rule("enum-to-enum"),
        (C::Alias, L::Julia) => Coverage::Rule("alias-to-const"),
        (C::Union, L::Julia) => Coverage::Rule("union-to-struct"),
        (C::Bitmask, L::Julia) => Coverage::Rule("bitmask-to-const"),
        (C::Bitset, L::Julia) => Coverage::Rule("bitset-to-struct"),
        (C::Valuetype, L::Julia) => Coverage::Rule("valuetype-to-struct"),
        (C::Const, L::Julia) => Coverage::Rule("const-to-const"),
        (C::Exception, L::Julia) => Coverage::Rule("exception-to-struct"),
        (C::Native, L::Julia) => Coverage::Rule("native-to-const"),
        (C::Interface, L::Julia) => Coverage::Rule("interface-to-abstract"),
        // The seven that are not declarations of their own: each becomes
        // a type at the point of use, which is the member rule's doing.
        (
            C::Primitive | C::Sequence | C::Array | C::StringType | C::Map | C::Fixed | C::ValueBox,
            L::Julia,
        ) => Coverage::Rule("member-to-field"),
        (C::ObjectRef, L::Julia) => Coverage::Uncovered("object references belong to the RPC arc"),
        (C::Any, L::Julia) => Coverage::Uncovered(
            "`any` carries its type at run time; this language has a type that would \
             hold it and no wire form behind it",
        ),
        (C::ValueBase, L::Julia) => Coverage::Uncovered(
            "which valuetype arrives is a run-time question; the CORBA arc owns it",
        ),
        (C::AnnotationType, L::Julia) => Coverage::Uncovered("an annotation declares no data"),

        // -- Lua ---------------------------------------------------------
        //
        // A module becomes a table, which is Lua's only scope.
        (C::Module, L::Lua) => Coverage::Rule("module-to-table"),
        (C::Structure, L::Lua) => Coverage::Rule("struct-to-table"),
        (C::Enumeration, L::Lua) => Coverage::Rule("enum-to-table"),
        (C::Alias, L::Lua) => Coverage::Rule("alias-to-entry"),
        (C::Union, L::Lua) => Coverage::Rule("union-to-table"),
        (C::Bitmask, L::Lua) => Coverage::Rule("bitmask-to-table"),
        (C::Bitset, L::Lua) => Coverage::Rule("bitset-to-table"),
        (C::Valuetype, L::Lua) => Coverage::Rule("valuetype-to-table"),
        (C::Const, L::Lua) => Coverage::Rule("const-to-field"),
        (C::Exception, L::Lua) => Coverage::Rule("exception-to-table"),
        (C::Native, L::Lua) => Coverage::Rule("native-to-table"),
        (C::Interface, L::Lua) => Coverage::Rule("interface-to-table"),
        // The seven that are not declarations of their own: each becomes
        // a type at the point of use, which is the member rule's doing.
        (
            C::Primitive | C::Sequence | C::Array | C::StringType | C::Map | C::Fixed | C::ValueBox,
            L::Lua,
        ) => Coverage::Rule("member-to-entry"),
        (C::ObjectRef, L::Lua) => Coverage::Uncovered("object references belong to the RPC arc"),
        (C::Any, L::Lua) => Coverage::Uncovered(
            "`any` carries its type at run time; this language has a type that would \
             hold it and no wire form behind it",
        ),
        (C::ValueBase, L::Lua) => Coverage::Uncovered(
            "which valuetype arrives is a run-time question; the CORBA arc owns it",
        ),
        (C::AnnotationType, L::Lua) => Coverage::Uncovered("an annotation declares no data"),

        // -- Nim ---------------------------------------------------------
        //
        // A module flattens with `_`: a Nim module is a file, so nested IDL
        // modules cannot nest in one generated file.
        (C::Module, L::Nim) => Coverage::Rule("module-to-prefix"),
        (C::Structure, L::Nim) => Coverage::Rule("struct-to-object"),
        (C::Enumeration, L::Nim) => Coverage::Rule("enum-to-enum"),
        (C::Alias, L::Nim) => Coverage::Rule("alias-to-alias"),
        (C::Union, L::Nim) => Coverage::Rule("union-to-object"),
        (C::Bitmask, L::Nim) => Coverage::Rule("bitmask-to-alias"),
        (C::Bitset, L::Nim) => Coverage::Rule("bitset-to-object"),
        (C::Valuetype, L::Nim) => Coverage::Rule("valuetype-to-object"),
        (C::Const, L::Nim) => Coverage::Rule("const-to-const"),
        (C::Exception, L::Nim) => Coverage::Rule("exception-to-object"),
        (C::Native, L::Nim) => Coverage::Rule("native-to-alias"),
        (C::Interface, L::Nim) => Coverage::Rule("interface-to-object"),
        // The seven that are not declarations of their own: each becomes
        // a type at the point of use, which is the member rule's doing.
        (
            C::Primitive | C::Sequence | C::Array | C::StringType | C::Map | C::Fixed | C::ValueBox,
            L::Nim,
        ) => Coverage::Rule("member-to-field"),
        (C::ObjectRef, L::Nim) => Coverage::Uncovered("object references belong to the RPC arc"),
        (C::Any, L::Nim) => Coverage::Uncovered(
            "`any` carries its type at run time; this language has a type that would \
             hold it and no wire form behind it",
        ),
        (C::ValueBase, L::Nim) => Coverage::Uncovered(
            "which valuetype arrives is a run-time question; the CORBA arc owns it",
        ),
        (C::AnnotationType, L::Nim) => Coverage::Uncovered("an annotation declares no data"),

        // -- Swift -------------------------------------------------------
        //
        // A module flattens with `_` rather than nesting in a caseless `enum`:
        // measured, an unqualified name inside a nested type resolves to the
        // nested one and compiles with no diagnostic, so two IDL scopes each
        // declaring one name would silently take the wrong type.
        (C::Module, L::Swift) => Coverage::Rule("module-to-prefix"),
        (C::Structure, L::Swift) => Coverage::Rule("struct-to-struct"),
        (C::Enumeration, L::Swift) => Coverage::Rule("enum-to-enum"),
        (C::Alias, L::Swift) => Coverage::Rule("alias-to-typealias"),
        (C::Union, L::Swift) => Coverage::Rule("union-to-struct"),
        (C::Bitmask, L::Swift) => Coverage::Rule("bitmask-to-optionset"),
        (C::Bitset, L::Swift) => Coverage::Rule("bitset-to-struct"),
        (C::Valuetype, L::Swift) => Coverage::Rule("valuetype-to-struct"),
        (C::Const, L::Swift) => Coverage::Rule("const-to-let"),
        (C::Exception, L::Swift) => Coverage::Rule("exception-to-struct"),
        (C::Native, L::Swift) => Coverage::Rule("native-to-typealias"),
        (C::Interface, L::Swift) => Coverage::Rule("interface-to-protocol"),
        // The seven that are not declarations of their own: each becomes
        // a type at the point of use, which is the member rule's doing.
        (
            C::Primitive | C::Sequence | C::Array | C::StringType | C::Map | C::Fixed | C::ValueBox,
            L::Swift,
        ) => Coverage::Rule("member-to-property"),
        (C::ObjectRef, L::Swift) => Coverage::Uncovered("object references belong to the RPC arc"),
        (C::Any, L::Swift) => Coverage::Uncovered(
            "`any` carries its type at run time; this language has a type that would \
             hold it and no wire form behind it",
        ),
        (C::ValueBase, L::Swift) => Coverage::Uncovered(
            "which valuetype arrives is a run-time question; the CORBA arc owns it",
        ),
        (C::AnnotationType, L::Swift) => Coverage::Uncovered("an annotation declares no data"),

        // -- Zig ---------------------------------------------------------
        //
        // A module flattens with `_`: a Zig file is a struct and has no
        // namespace a declaration can be reopened in.
        (C::Module, L::Zig) => Coverage::Rule("module-to-prefix"),
        (C::Structure, L::Zig) => Coverage::Rule("struct-to-struct"),
        (C::Enumeration, L::Zig) => Coverage::Rule("enum-to-enum"),
        (C::Alias, L::Zig) => Coverage::Rule("alias-to-const"),
        (C::Union, L::Zig) => Coverage::Rule("union-to-struct"),
        (C::Bitmask, L::Zig) => Coverage::Rule("bitmask-to-int"),
        (C::Bitset, L::Zig) => Coverage::Rule("bitset-to-struct"),
        (C::Valuetype, L::Zig) => Coverage::Rule("valuetype-to-struct"),
        (C::Const, L::Zig) => Coverage::Rule("const-to-const"),
        (C::Exception, L::Zig) => Coverage::Rule("exception-to-struct"),
        (C::Native, L::Zig) => Coverage::Rule("native-to-opaque"),
        (C::Interface, L::Zig) => Coverage::Rule("interface-to-struct"),
        // The seven that are not declarations of their own: each becomes
        // a type at the point of use, which is the member rule's doing.
        (
            C::Primitive | C::Sequence | C::Array | C::StringType | C::Map | C::Fixed | C::ValueBox,
            L::Zig,
        ) => Coverage::Rule("member-to-field"),
        (C::ObjectRef, L::Zig) => Coverage::Uncovered("object references belong to the RPC arc"),
        (C::Any, L::Zig) => {
            Coverage::Uncovered("`any` carries its type at run time; the CORBA arc owns it")
        }
        (C::ValueBase, L::Zig) => Coverage::Uncovered(
            "which valuetype arrives is a run-time question; the CORBA arc owns it",
        ),
        (C::AnnotationType, L::Zig) => Coverage::Uncovered("an annotation declares no data"),

        // -- Node (plain JavaScript) ---------------------------------------
        //
        // This column's materialiser (`crates/lang-rules/src/adapter/node.rs`)
        // declares almost nothing: a struct, a union, a bitset and a
        // valuetype are all plain object literals wherever the codec walk
        // or the Table 9 defaulter builds one, needing no named
        // declaration at all — "the runtime carries the shape" is the
        // adapter's own words for it. What is covered below is a
        // construct whose *value* is correctly represented wherever it is
        // built; what is uncovered is a construct nothing here ever needs
        // to name, because it is never a struct member's type in any
        // corpus case and a dynamically-typed column has no consumer that
        // would look one up by a declared name if it were.
        //
        // Three seams `crates/lang-rules/src/emit/node.rs` need no
        // override for, because plain JavaScript's own semantics already
        // answer what a typed sibling has to spell out. `bool_less` is
        // the language's own `<`: it coerces `false` and `true` to 0 and
        // 1 correctly, where TypeScript's compiler refuses the
        // comparison outright (`crates/lang-rules/src/emit/typescript.rs`'s
        // `bool_less` exists only for that refusal). A property name is
        // never escaped, because IDL 4.2 §7.2.3's identifier grammar is a
        // strict subset of ECMA-262 §12.6.1's `IdentifierName`, which has
        // admitted every reserved word as a property key since
        // ECMAScript 5 — `zerodds_lang_cst::node::escape_member` is the
        // identity, always. And `@external` needs no indirection at all,
        // because every object this column builds is already a
        // reference: none of the `external_*` seams are overridden, the
        // same as TypeScript's.
        (C::Module, L::Node) => Coverage::Rule("module-to-flattened-name"),
        (C::Structure, L::Node) => Coverage::Rule("struct-to-object-literal"),
        (C::Union, L::Node) => Coverage::Rule("union-to-object-literal"),
        (C::Enumeration, L::Node) => Coverage::Rule("enum-to-frozen-object"),
        (C::Bitmask, L::Node) => Coverage::Rule("bit-to-const"),
        (C::Bitset, L::Node) => Coverage::Rule("bitset-to-object-literal"),
        // §7.4.1.4.3: an alias "does not define a new type", and this is
        // the one column where that is not merely the clause but the
        // whole of the mapping — a typedef'd member takes its resolved
        // target's representation directly, with no declaration standing
        // between them for a name to escape onto.
        (C::Alias, L::Node) => Coverage::Rule("alias-transparent"),
        (C::Valuetype, L::Node) => Coverage::Rule("valuetype-to-object-literal"),
        (C::Const, L::Node) => Coverage::Rule("const-to-const"),
        // The seven that are not declarations of their own: each is
        // written where the member is, as a `number`, a `bigint`, a
        // one-character `string`, an `Array` or a plain object property
        // — `crates/lang-rules/src/emit/node.rs`'s module documentation
        // carries the boundary.
        (
            C::Primitive | C::Sequence | C::Array | C::StringType | C::Map | C::Fixed | C::ValueBox,
            L::Node,
        ) => Coverage::Rule("member-to-property"),
        (C::Native, L::Node) => Coverage::Uncovered(
            "opaque and never a member's type in the corpus; a dynamically-typed \
             column has no consumer that would look one up by a declared name",
        ),
        (C::Exception, L::Node) => Coverage::Uncovered(
            "an exception is never a struct member's type, only an operation's \
             raises clause; nothing here declares a name for one",
        ),
        (C::Interface, L::Node) => Coverage::Uncovered(
            "an interface is the RPC arc's and not the wire's, and nothing here \
             declares a name for one",
        ),
        (C::ObjectRef, L::Node) => Coverage::Uncovered("object references belong to the RPC arc"),
        (C::Any, L::Node) => {
            Coverage::Uncovered("`any` carries its type at run time; the CORBA arc owns it")
        }
        (C::ValueBase, L::Node) => Coverage::Uncovered(
            "which valuetype arrives is a run-time question; the CORBA arc owns it",
        ),
        (C::AnnotationType, L::Node) => Coverage::Uncovered("an annotation declares no data"),

        // -- Ada 83 --------------------------------------------------------
        //
        // A module flattens exactly as `adapter::ada`'s does, for the
        // same reason: GNAT ties a compilation unit to a file name.
        // Unlike `adapter::ada`, an interface has no rule at all here —
        // RM83 has no interface type, no tagged type and no `abstract`,
        // and `ada83_items` refuses one by name before any per-construct
        // rule of the catalogue would be consulted for it, so no ghost
        // kind is declared for one (see `rule::ada83`'s module
        // documentation). An enumerator is `enum-to-constants`, not
        // `enum-to-type`: RM83 has no confirmed `'Enum_Rep`/`'Enum_Val`
        // under `-gnat83`, so an enumerator is a named `Long_Integer`
        // constant, the same shape a bitmask's bits already have.
        (C::Module, L::Ada83) => Coverage::Rule("module-to-prefix"),
        (C::Structure, L::Ada83) => Coverage::Rule("struct-to-record"),
        (C::Enumeration, L::Ada83) => Coverage::Rule("enum-to-constants"),
        (C::Alias, L::Ada83) => Coverage::Rule("alias-to-subtype"),
        (C::Union, L::Ada83) => Coverage::Rule("union-to-record"),
        (C::Bitmask, L::Ada83) => Coverage::Rule("bitmask-to-type"),
        (C::Bitset, L::Ada83) => Coverage::Rule("bitset-to-record"),
        (C::Valuetype, L::Ada83) => Coverage::Rule("valuetype-to-record"),
        (C::Const, L::Ada83) => Coverage::Rule("const-to-constant"),
        (C::Exception, L::Ada83) => Coverage::Rule("exception-to-record"),
        (C::Native, L::Ada83) => Coverage::Rule("native-to-type"),
        // The seven that are not declarations of their own: each becomes
        // a type at the point of use, which is the member rule's doing,
        // exactly as in `adapter::ada`.
        (
            C::Primitive | C::Sequence | C::Array | C::StringType | C::Map | C::Fixed | C::ValueBox,
            L::Ada83,
        ) => Coverage::Rule("member-to-component"),
        (C::Interface, L::Ada83) => Coverage::Uncovered(
            "RM83 has no interface type, no tagged type and no `abstract` primitive \
             operation — all three are Ada 95 or Ada 2005 additions — so `ada83_items` \
             refuses one by name before any rule of this catalogue is consulted",
        ),
        (C::ObjectRef, L::Ada83) => Coverage::Uncovered("object references belong to the RPC arc"),
        (C::Any, L::Ada83) => {
            Coverage::Uncovered("`any` carries its type at run time; the CORBA arc owns it")
        }
        (C::ValueBase, L::Ada83) => Coverage::Uncovered(
            "which valuetype arrives is a run-time question; the CORBA arc owns it",
        ),
        (C::AnnotationType, L::Ada83) => Coverage::Uncovered("an annotation declares no data"),

        // -- F# --------------------------------------------------------
        //
        // The same shape `rule::fsharp`'s module documentation gives
        // beside `rule::zig`'s: a module flattens onto the name, a
        // union is a class with a discriminator rather than F#'s own
        // discriminated union (`zerodds_lang_cst::fsharp`'s module
        // documentation carries why), and a bitmask still needs
        // file-scope constants beside its type abbreviation.
        (C::Module, L::FSharp) => Coverage::Rule("module-to-prefix"),
        (C::Structure, L::FSharp) => Coverage::Rule("struct-to-class"),
        (C::Enumeration, L::FSharp) => Coverage::Rule("enum-to-enum"),
        (C::Alias, L::FSharp) => Coverage::Rule("alias-to-abbreviation"),
        (C::Union, L::FSharp) => Coverage::Rule("union-to-class"),
        (C::Bitmask, L::FSharp) => Coverage::Rule("bitmask-to-int"),
        (C::Bitset, L::FSharp) => Coverage::Rule("bitset-to-class"),
        (C::Valuetype, L::FSharp) => Coverage::Rule("valuetype-to-class"),
        (C::Const, L::FSharp) => Coverage::Rule("const-to-const"),
        (C::Exception, L::FSharp) => Coverage::Rule("exception-to-class"),
        (C::Interface, L::FSharp) => Coverage::Rule("interface-to-class"),
        (C::Native, L::FSharp) => Coverage::Rule("native-to-opaque"),
        // The seven that are not declarations of their own: each becomes
        // a type at the point of use, which is `member-to-field`'s
        // doing, as in C#.
        (
            C::Primitive | C::Sequence | C::Array | C::StringType | C::Map | C::Fixed | C::ValueBox,
            L::FSharp,
        ) => Coverage::Rule("member-to-field"),
        (C::Any, L::FSharp) => Coverage::Uncovered(
            "an `any` needs the dynamic type system, which is the reflective arc",
        ),
        (C::ValueBase, L::FSharp) => {
            Coverage::Uncovered("no F# form; a valuetype base is not a declaration of its own")
        }
        (C::ObjectRef, L::FSharp) => Coverage::Uncovered("object references belong to the RPC arc"),
        (C::AnnotationType, L::FSharp) => Coverage::Uncovered("an annotation declares no data"),

        // -- Kotlin ----------------------------------------------------
        //
        // Kotlin shares the JVM's value model with Java, but Java's own
        // columns above are still "not yet written" placeholders — this
        // column was built fresh (`docs/language-columns-status.md`).
        // An enumeration is `enum-to-object`, not an `enum class`:
        // Kotlin's `enum class` fixes a constant to its declaration
        // ordinal, which cannot reproduce the out-of-order or gapped
        // `@value` DDS-XTypes 1.3 §7.4.3.5.3 admits, so an enumeration
        // is an `object` of `const val Int` instead — the same argument
        // `adapter::java` gives for `public static final int`. A
        // bitmask is `bitmask-to-alias` in name only: a Kotlin
        // `typealias` is transparent to the compiler, so the rule's
        // declaration is the bits' constants and the holder type is
        // used directly, with no aggregate of its own.
        (C::Module, L::Kotlin) => Coverage::Rule("module-to-prefix"),
        (C::Structure, L::Kotlin) => Coverage::Rule("struct-to-class"),
        (C::Enumeration, L::Kotlin) => Coverage::Rule("enum-to-object"),
        (C::Alias, L::Kotlin) => Coverage::Rule("alias-to-alias"),
        (C::Union, L::Kotlin) => Coverage::Rule("union-to-class"),
        (C::Bitmask, L::Kotlin) => Coverage::Rule("bitmask-to-alias"),
        (C::Bitset, L::Kotlin) => Coverage::Rule("bitset-to-class"),
        (C::Valuetype, L::Kotlin) => Coverage::Rule("valuetype-to-class"),
        (C::Const, L::Kotlin) => Coverage::Rule("const-to-const"),
        (C::Exception, L::Kotlin) => Coverage::Rule("exception-to-class"),
        (C::Interface, L::Kotlin) => Coverage::Rule("interface-to-class"),
        (C::Native, L::Kotlin) => Coverage::Rule("native-to-alias"),
        // The seven that are not declarations of their own: each becomes
        // a type at the point of use, which is `member-to-property`'s
        // doing.
        (
            C::Primitive | C::Sequence | C::Array | C::StringType | C::Map | C::Fixed | C::ValueBox,
            L::Kotlin,
        ) => Coverage::Rule("member-to-property"),
        (C::Any, L::Kotlin) => Coverage::Uncovered(
            "an `any` needs the dynamic type system, which is the reflective arc",
        ),
        (C::ValueBase, L::Kotlin) => {
            Coverage::Uncovered("no Kotlin form; a valuetype base is not a declaration of its own")
        }
        (C::ObjectRef, L::Kotlin) => Coverage::Uncovered("object references belong to the RPC arc"),
        (C::AnnotationType, L::Kotlin) => Coverage::Uncovered("an annotation declares no data"),
    }
}

/// How many cells of the table a rule covers.
#[must_use]
pub fn covered_count() -> usize {
    let mut count = 0;
    for construct in ConstructKind::ALL {
        for language in Language::ALL {
            if coverage_of(construct, language).is_covered() {
                count += 1;
            }
        }
    }
    count
}

/// How many cells the table has.
#[must_use]
pub const fn cell_count() -> usize {
    ConstructKind::ALL.len() * Language::ALL.len()
}
