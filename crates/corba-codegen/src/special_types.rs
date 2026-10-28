// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The thirteen Annex A.1 special types, and what each language PSM
//! calls them.
//!
//! CORBA 3.3 part 1 Annex A.1 lists the IDL constructs whose language
//! mapping is not the mechanical one. Everything else — a `long`, a
//! `struct`, a `sequence<long>` — takes the ordinary rule its PSM
//! defines. These thirteen do not, and a generator that treats them as
//! ordinary emits a name that does not exist.
//!
//! ## Spec sources, per column
//!
//! | column | document |
//! |---|---|
//! | C++ | IDL to C++, `formal/2008-01-09`, §1.4–§1.16 and Table 1-1 |
//! | Java | IDL to Java, `formal/2008-01-04`, §1.3 and Table 1-1 |
//! | C# | IDL to C#, `formal/2003-09-04`, Annex A |
//! | Rust | no OMG mapping exists; this project's, and named as such |
//!
//! ## Where the Rust column comes from
//!
//! Three of its rows were decided in this rebuild and not by the earlier
//! table, which named crates that no longer carry the type:
//!
//! - **`long double` is [`Float128`] and not `f64`.** `zerodds-float128`
//!   carries the sixteen bytes CDR gives it (CORBA 3.3 part 2 §9.3.1.3).
//!   `f64` put eight bytes of value and eight of padding on the wire —
//!   the right length and a number nobody wrote.
//!   `zerodds_lang_rules::adapter::cst` reaches the same answer for a
//!   generated Rust structure, and these two must not disagree.
//! - **`any`, `TypeCode` and `wstring` are `zerodds-corba-rt`'s.** They
//!   moved there in step 1 of the rebuild, out of the wire crate: a
//!   `TypeCode` is a self-describing type graph and `zerodds-wire` is a
//!   byte runtime.
//! - **`fixed<P,S>` has no Rust holder in this tree.** `zerodds-cdr`
//!   carried one and was demolished; `zerodds-wire` has none, and
//!   `zerodds_lang_rules::adapter::cst` writes a bare `Fixed` that
//!   resolves to whatever the generated module brings. The row says
//!   `Fixed` and [`SpecialType::Fixed`]'s documentation says why, rather
//!   than naming a path that does not resolve.
//!
//! ## Where the C++ and Java columns come from
//!
//! The OMG's documents, and then `omniidl`, `tao_idl` and JacORB's `idl`,
//! which were asked what they generate for each row in each argument
//! position. Four answers changed as a result and
//! `tests/orb_agreement.rs` names them. The C# column has an OMG mapping
//! and no compiler on the measurement host: it is cited and **not**
//! measured, and this says so rather than letting it pass as checked.
//!
//! [`Float128`]: https://docs.rs/zerodds-float128

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_ast::{Ast, CharWidth, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_cst::{Language, Path, cpp, csharp, java, rust};

/// Which language a mapping is asked for.
///
/// The four PSM columns Annex A.1 has an answer for. It is not
/// [`zerodds_lang_cst::Language`]: that enum is the set of trees this
/// project can build — Rust, C++, C and Java — and the two sets differ
/// in both directions. C has no CORBA PSM at all, and C# has an OMG
/// mapping but no CST here. [`TargetLanguage::cst_language`] and
/// [`TargetLanguage::from_cst_language`] carry the overlap and report the
/// rest rather than defaulting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetLanguage {
    /// C++ — IDL to C++, `formal/2008-01-09`.
    Cpp,
    /// C# / .NET — IDL to C#, `formal/2003-09-04`.
    CSharp,
    /// Java — IDL to Java, `formal/2008-01-04`.
    Java,
    /// Rust — this project's mapping. The OMG has none.
    Rust,
}

impl TargetLanguage {
    /// Every column, in the order the tables are written.
    pub const ALL: [Self; 4] = [Self::Cpp, Self::CSharp, Self::Java, Self::Rust];

    /// The columns a CST can be built for, so a helper that renders text
    /// can be exhaustive over them.
    ///
    /// All four. It was three until `zerodds_lang_cst::csharp` arrived:
    /// the C# column had an OMG mapping and no tree to write it into, so
    /// [`crate::stub::render_stub_op`] and
    /// [`crate::skeleton::render_skeleton_dispatch`] answered
    /// [`crate::NoRenderer`] for it. The tree exists now, and the
    /// refusal went with it.
    pub const RENDERABLE: [Self; 4] = [Self::Cpp, Self::CSharp, Self::Java, Self::Rust];

    /// The tree this column is rendered through.
    ///
    /// `Some` for all four. The variant is where a module registers
    /// itself and having a tree is a second question; this answers the
    /// second one, and it answered `None` for C# until
    /// `zerodds_lang_cst::csharp` held one. The test below is what makes
    /// that a decision rather than an oversight, in both directions.
    #[must_use]
    pub const fn cst_language(self) -> Option<Language> {
        match self {
            Self::Cpp => Some(Language::Cpp),
            Self::Java => Some(Language::Java),
            Self::Rust => Some(Language::Rust),
            Self::CSharp => Some(Language::CSharp),
        }
    }

    /// The column a CST language belongs to, where it has one.
    ///
    /// `None` for every CST language the OMG published no CORBA PSM for —
    /// C, Python, TypeScript and the ten the rebuild brought back. The
    /// mirror of [`Self::cst_language`]. A caller iterating the CST
    /// languages is told which ones have no answer instead of receiving
    /// C++'s.
    ///
    /// The arms are written out rather than closed with a wildcard, and
    /// the compiler enforcing that is the point: `Language::ALL` went
    /// from seven to seventeen and this match was the one place in the
    /// workspace that failed to build. A `_ => None` here would have
    /// taken ten new languages silently, and the first CORBA PSM the OMG
    /// publishes for one of them would arrive as a `None` nobody was
    /// told about.
    #[must_use]
    pub const fn from_cst_language(language: Language) -> Option<Self> {
        match language {
            Language::Cpp => Some(Self::Cpp),
            Language::Java => Some(Self::Java),
            Language::Rust => Some(Self::Rust),
            Language::CSharp => Some(Self::CSharp),
            Language::C
            | Language::Python
            | Language::TypeScript
            | Language::Go
            | Language::OCaml
            | Language::Ada
            | Language::D
            | Language::Elixir
            | Language::Julia
            | Language::Lua
            | Language::Nim
            | Language::Swift
            | Language::Zig
            | Language::Node
            // Ada 83 is the language the OMG's Ada PSM does *not* cover:
            // that mapping is written against Ada 95 and uses tagged
            // types and child units, neither of which RM83 has. The
            // 2012 column above answers `None` for its own reason — no
            // PSM tree here — and this one would answer `None` even if
            // that tree existed.
            | Language::Ada83
            | Language::FSharp
            | Language::Kotlin => None,
        }
    }

    /// How the column is written in a diagnostic.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cpp => "C++",
            Self::CSharp => "C#",
            Self::Java => "Java",
            Self::Rust => "Rust",
        }
    }

    /// The separator this language writes between name segments.
    ///
    /// C++ scopes with `::` and the other three with `.`, which is why
    /// [`language_mapping`]'s strings can be split back into a
    /// [`Path`] without a second table.
    #[must_use]
    pub const fn scope_separator(self) -> &'static str {
        match self {
            // Rust scopes with `::` like C++; C# and Java with `.`.
            Self::Cpp | Self::Rust => "::",
            Self::CSharp | Self::Java => ".",
        }
    }
}

/// The thirteen special types of Annex A.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpecialType {
    /// `Object` — the generic object reference. IDL 4.2 rule (117),
    /// CORBA 3.3 part 1 §7.4.6.
    Object,
    /// `ValueBase` — the base every valuetype has. IDL 4.2 rule (131).
    ValueBase,
    /// `CORBA::AbstractBase` — the root of the abstract-interface
    /// hierarchy. IDL 4.2 §7.4.6.
    ///
    /// It has no IDL production, and no `abstract interface` reaches it:
    /// a declared one is written by its own name in all three ORBs
    /// (`tests/orb-corpus/recorded/*`, row 03). Like
    /// [`SpecialType::TypeCode`] it is answered by
    /// [`special_type_of_scoped`] and never by [`special_type_of`].
    AbstractBase,
    /// A `native` declaration used as a type. IDL 4.2 §7.4.11.
    NativeRef,
    /// `TypeCode`. CORBA 3.3 part 2 §15.3.5.
    TypeCode,
    /// `any`. IDL 4.2 §7.4.2, CORBA 3.3 part 2 §15.3.7.
    Any,
    /// `sequence<any>`.
    ///
    /// A row of its own rather than a sequence of the row above, because
    /// C++ and C# give it a named type — `CORBA::AnySeq` — where the
    /// generic sequence mapping would have produced an instantiation.
    /// Java does not: measured, JacORB writes `org.omg.CORBA.Any[]`,
    /// which is §1.3.6's ordinary array mapping applied to the element
    /// type (`tests/orb-corpus/recorded/jacorb.txt`, row 07).
    SequenceOfAny,
    /// `string`. IDL 4.2 §7.4.1.4.
    String,
    /// `wstring`. IDL 4.2 §7.4.1.5, CORBA 3.3 part 2 §15.3.2.7.
    WString,
    /// `TimeBase::TimeT`. Time Service, `formal/2002-06-06` §3.1.2.
    ///
    /// Declared IDL and not a built-in: it is a `typedef unsigned long
    /// long TimeT` inside `module TimeBase`. So [`special_type_of`]
    /// cannot answer it from a type node alone — a model carries it as
    /// an alias — and [`special_type_of_scoped`] takes the name.
    Time,
    /// `fixed<digits, scale>`. IDL 4.2 §7.4.1.7 — at most 31 digits.
    Fixed,
    /// `unsigned long long`. IDL 4.2 §7.4.1.1.
    ULongLong,
    /// `long double`. IDL 4.2 §7.4.1.2, CORBA 3.3 part 2 §9.3.1.3.
    LongDouble,
}

impl SpecialType {
    /// The IDL spelling, as Annex A.1 writes it.
    #[must_use]
    pub const fn idl_spelling(self) -> &'static str {
        match self {
            Self::Object => "Object",
            Self::ValueBase => "ValueBase",
            Self::AbstractBase => "AbstractBase",
            Self::NativeRef => "native",
            Self::TypeCode => "TypeCode",
            Self::Any => "any",
            Self::SequenceOfAny => "sequence<any>",
            Self::String => "string",
            Self::WString => "wstring",
            Self::Time => "TimeBase::TimeT",
            Self::Fixed => "fixed<digits,scale>",
            Self::ULongLong => "unsigned long long",
            Self::LongDouble => "long double",
        }
    }
}

/// All thirteen, in Annex A.1 order.
#[must_use]
pub const fn all_special_types() -> [SpecialType; 13] {
    [
        SpecialType::Object,
        SpecialType::ValueBase,
        SpecialType::AbstractBase,
        SpecialType::NativeRef,
        SpecialType::TypeCode,
        SpecialType::Any,
        SpecialType::SequenceOfAny,
        SpecialType::String,
        SpecialType::WString,
        SpecialType::Time,
        SpecialType::Fixed,
        SpecialType::ULongLong,
        SpecialType::LongDouble,
    ]
}

/// What a language calls a special type **where a value of it is held**
/// — a structure member, a local, a valuetype's state.
///
/// Not what a parameter of that type is written as. IDL to C++ §1.22
/// gives `in`, `inout`, `out` and the return position four further
/// spellings built from the same name, and this table's C++ column
/// carries the `_var` managed forms of §1.4.2 rather than any of them.
/// [`crate::stub::cpp_param_type`] answers the argument positions, and
/// its answers are what the recorded ORB output is checked against —
/// an ORB writes a signature and not a member declaration.
///
/// One table and no second copy: [`cpp_type`], [`java_type`] and
/// [`rust_type`] all split this string on the language's scope
/// separator. A CST node and a documentation string that disagreed
/// would be the shape of defect this rebuild keeps finding.
#[must_use]
pub const fn language_mapping(t: SpecialType, lang: TargetLanguage) -> &'static str {
    match (t, lang) {
        // --- C++ (formal/2008-01-09) -------------------------------
        // The `_var` forms are the managed-reference types §1.4.2
        // defines; a generated member holds one of those and not a raw
        // `_ptr`, which owns nothing.
        (SpecialType::Object, TargetLanguage::Cpp) => "CORBA::Object_var",
        (SpecialType::ValueBase, TargetLanguage::Cpp) => "CORBA::ValueBase",
        (SpecialType::AbstractBase, TargetLanguage::Cpp) => "CORBA::AbstractBase_var",
        (SpecialType::NativeRef, TargetLanguage::Cpp) => "void*",
        (SpecialType::TypeCode, TargetLanguage::Cpp) => "CORBA::TypeCode_var",
        (SpecialType::Any, TargetLanguage::Cpp) => "CORBA::Any",
        (SpecialType::SequenceOfAny, TargetLanguage::Cpp) => "CORBA::AnySeq",
        (SpecialType::String, TargetLanguage::Cpp) => "CORBA::String_var",
        (SpecialType::WString, TargetLanguage::Cpp) => "CORBA::WString_var",
        (SpecialType::Time, TargetLanguage::Cpp) => "TimeBase::TimeT",
        (SpecialType::Fixed, TargetLanguage::Cpp) => "CORBA::Fixed",
        (SpecialType::ULongLong, TargetLanguage::Cpp) => "CORBA::ULongLong",
        (SpecialType::LongDouble, TargetLanguage::Cpp) => "CORBA::LongDouble",

        // --- C# (formal/2003-09-04) --------------------------------
        (SpecialType::Object, TargetLanguage::CSharp) => "omg.org.CORBA.Object",
        (SpecialType::ValueBase, TargetLanguage::CSharp) => "omg.org.CORBA.ValueBase",
        (SpecialType::AbstractBase, TargetLanguage::CSharp) => "omg.org.CORBA.AbstractBase",
        (SpecialType::NativeRef, TargetLanguage::CSharp) => "object",
        (SpecialType::TypeCode, TargetLanguage::CSharp) => "omg.org.CORBA.TypeCode",
        (SpecialType::Any, TargetLanguage::CSharp) => "omg.org.CORBA.Any",
        (SpecialType::SequenceOfAny, TargetLanguage::CSharp) => "omg.org.CORBA.AnySeq",
        (SpecialType::String, TargetLanguage::CSharp) => "string",
        (SpecialType::WString, TargetLanguage::CSharp) => "string",
        (SpecialType::Time, TargetLanguage::CSharp) => "ulong",
        (SpecialType::Fixed, TargetLanguage::CSharp) => "decimal",
        (SpecialType::ULongLong, TargetLanguage::CSharp) => "ulong",
        // `decimal` and not `double`: C# has no 128-bit binary float,
        // and a `double` would drop the eight low bytes of the sixteen
        // CDR gives a `long double`. `decimal` is 128 bits and holds
        // the value the wire carried, at the cost of a base change.
        (SpecialType::LongDouble, TargetLanguage::CSharp) => "decimal",

        // --- Java (formal/2008-01-04) ------------------------------
        (SpecialType::Object, TargetLanguage::Java) => "org.omg.CORBA.Object",
        // Not `org.omg.CORBA.ValueBase`: §1.3.7 maps the IDL
        // `ValueBase` onto `java.io.Serializable`, which is what
        // every generated valuetype implements.
        (SpecialType::ValueBase, TargetLanguage::Java) => "java.io.Serializable",
        (SpecialType::AbstractBase, TargetLanguage::Java) => "org.omg.CORBA.portable.IDLEntity",
        (SpecialType::NativeRef, TargetLanguage::Java) => "java.lang.Object",
        (SpecialType::TypeCode, TargetLanguage::Java) => "org.omg.CORBA.TypeCode",
        (SpecialType::Any, TargetLanguage::Java) => "org.omg.CORBA.Any",
        // `Any[]` and not `AnySeqHelper`. §1.3.6 maps an IDL sequence
        // onto a Java array of the element type; a `Helper` is the
        // marshalling class beside it and is never a member type.
        // Measured: JacORB 3.9 writes `void in_v(org.omg.CORBA.Any[] v)`
        // — `tests/orb-corpus/recorded/jacorb.txt`, row 07.
        (SpecialType::SequenceOfAny, TargetLanguage::Java) => "org.omg.CORBA.Any[]",
        (SpecialType::String, TargetLanguage::Java) => "java.lang.String",
        (SpecialType::WString, TargetLanguage::Java) => "java.lang.String",
        (SpecialType::Time, TargetLanguage::Java) => "long",
        (SpecialType::Fixed, TargetLanguage::Java) => "java.math.BigDecimal",
        (SpecialType::ULongLong, TargetLanguage::Java) => "long",
        // Java has no 128-bit float either. §1.3.1 maps `long double`
        // onto `double`, and the mapping says outright that the value
        // may not survive; the row records the OMG's answer rather
        // than substituting a better one the ORBs would not agree
        // with.
        (SpecialType::LongDouble, TargetLanguage::Java) => "double",

        // --- Rust (this project's mapping) -------------------------
        (SpecialType::Object, TargetLanguage::Rust) => "zerodds_corba_rt::ObjectReference",
        (SpecialType::ValueBase, TargetLanguage::Rust) => "zerodds_corba_rt::ValueBase",
        (SpecialType::AbstractBase, TargetLanguage::Rust) => "zerodds_corba_rt::ValueBase",
        (SpecialType::NativeRef, TargetLanguage::Rust) => "core::ffi::c_void",
        (SpecialType::TypeCode, TargetLanguage::Rust) => "zerodds_corba_rt::TypeCode",
        (SpecialType::Any, TargetLanguage::Rust) => "zerodds_corba_rt::CorbaAny",
        (SpecialType::SequenceOfAny, TargetLanguage::Rust) => "Vec<zerodds_corba_rt::CorbaAny>",
        (SpecialType::String, TargetLanguage::Rust) => "String",
        // `zerodds_corba_rt::WString` and not `String`: §15.3.2.7's
        // byte-order mark and code-unit order are carried on the type,
        // and a `String` has nowhere to put them.
        (SpecialType::WString, TargetLanguage::Rust) => "zerodds_corba_rt::WString",
        (SpecialType::Time, TargetLanguage::Rust) => "u64",
        // No holder in this tree; see the module documentation.
        (SpecialType::Fixed, TargetLanguage::Rust) => "Fixed",
        (SpecialType::ULongLong, TargetLanguage::Rust) => "u64",
        (SpecialType::LongDouble, TargetLanguage::Rust) => "zerodds_float128::Float128",
    }
}

/// Which special type a model type node is, if it is one.
///
/// The connection to the rebuilt front end: a generator holds an
/// [`Id<TypeNode>`] out of `zerodds-idl-lower` and asks this rather than
/// matching on [`TypeKind`] itself, so the thirteen rows have one reader.
///
/// Three rows are not reachable from a type node and answer `None` here:
///
/// - [`SpecialType::Time`] is a declared `typedef` in `module TimeBase`,
///   so it arrives as [`TypeKind::Alias`]. [`special_type_of_scoped`]
///   takes the scoped name a caller already has.
/// - [`SpecialType::TypeCode`] has no IDL type production at all — it is
///   reachable only through an `any`'s runtime content and through the
///   `TypeCode` pseudo-object in the `CORBA` module.
/// - [`SpecialType::AbstractBase`] is answered for an object reference to
///   an `abstract interface`, which is the only IDL that reaches it.
///
/// A `typedef` is followed: `typedef string Name;` used as a member type
/// is still `string`, and a generator that stopped at the alias would
/// miss the row.
///
/// zerodds-lint: recursion-depth = declared alias and `sequence<>`
/// nesting. A struct, union or valuetype is a leaf here — it answers
/// `None` without a descent into its members — and a cycle in the type
/// graph can only run through one of those, because a `typedef`'s
/// target and a `sequence<>`'s element must already be declared where
/// they are written. So this walk descends declaration nesting only,
/// which the source text bounds.
#[must_use]
pub fn special_type_of(ast: &Ast, id: Id<TypeNode>) -> Option<SpecialType> {
    let node = ast.type_(id).ok()?;
    match &node.kind {
        TypeKind::Primitive(PrimitiveKind::UInt64) => Some(SpecialType::ULongLong),
        TypeKind::Primitive(PrimitiveKind::Float128) => Some(SpecialType::LongDouble),
        TypeKind::Fixed { .. } => Some(SpecialType::Fixed),
        TypeKind::String {
            width: CharWidth::Char,
            ..
        } => Some(SpecialType::String),
        TypeKind::String {
            width: CharWidth::WChar,
            ..
        } => Some(SpecialType::WString),
        TypeKind::Any => Some(SpecialType::Any),
        TypeKind::ValueBase => Some(SpecialType::ValueBase),
        TypeKind::Native => Some(SpecialType::NativeRef),
        // Only the *generic* `Object` is a row. A reference to a named
        // interface — concrete or abstract — takes that interface's own
        // name, which all three ORBs confirm: for
        // `abstract interface Abstracted`, omniidl and tao_idl write
        // `::probe::Abstracted_ptr` and JacORB `probe.Abstracted`, none
        // of them `CORBA::AbstractBase` or `IDLEntity`
        // (`tests/orb-corpus/recorded/*`, row 03). An earlier version
        // answered [`SpecialType::AbstractBase`] here and would have
        // produced `CORBA::AbstractBase_ptr` where every ORB writes the
        // declared name.
        TypeKind::ObjectRef { interface } => interface.is_none().then_some(SpecialType::Object),
        // Only a sequence *of `any`* is a row; every other element type
        // takes the PSM's generic sequence mapping.
        TypeKind::Sequence { element, .. } => {
            let element = element.resolved()?;
            matches!(special_type_of(ast, element)?, SpecialType::Any)
                .then_some(SpecialType::SequenceOfAny)
        }
        TypeKind::Alias { aliased } => special_type_of(ast, aliased.resolved()?),
        _ => None,
    }
}

/// The special type a scoped IDL name denotes, for the two rows that are
/// declared IDL rather than a production.
///
/// `TimeBase::TimeT` and `CORBA::TypeCode` are written as names and are
/// resolved like any other name, so a type node alone cannot answer for
/// them — by the time lowering is done, `TimeT` is an alias of `unsigned
/// long long` and indistinguishable from one a user wrote.
///
/// Segments are taken as written, with a leading `::` accepted and
/// ignored.
#[must_use]
pub fn special_type_of_scoped(segments: &[&str]) -> Option<SpecialType> {
    let segments: Vec<&str> = segments
        .iter()
        .copied()
        .filter(|segment| !segment.is_empty())
        .collect();
    match segments.as_slice() {
        ["TimeBase", "TimeT"] => Some(SpecialType::Time),
        ["CORBA", "TypeCode"] | ["TypeCode"] => Some(SpecialType::TypeCode),
        ["CORBA", "Object"] | ["Object"] => Some(SpecialType::Object),
        ["CORBA", "ValueBase"] | ["ValueBase"] => Some(SpecialType::ValueBase),
        ["CORBA", "AbstractBase"] | ["AbstractBase"] => Some(SpecialType::AbstractBase),
        _ => None,
    }
}

/// The C++ type node for a special type.
#[must_use]
pub fn cpp_type(t: SpecialType) -> cpp::Type {
    let written = language_mapping(t, TargetLanguage::Cpp);
    // `native` is the one row that is not a name: it is a pointer, and
    // a `Named` node holding the text `void*` would render as an
    // identifier containing a star.
    if written == "void*" {
        return cpp::Type::Pointer(alloc::boxed::Box::new(cpp::Type::Void));
    }
    cpp::Type::named(path_of(written, "::"))
}

/// The Java type node for a special type.
#[must_use]
pub fn java_type(t: SpecialType) -> java::Type {
    let written = language_mapping(t, TargetLanguage::Java);
    if let Some(element) = written.strip_suffix("[]") {
        return java::Type::Array(alloc::boxed::Box::new(java::Type::named(path_of(
            element, ".",
        ))));
    }
    match written {
        "double" => java::Type::Primitive(java::Primitive::Double),
        "long" => java::Type::Primitive(java::Primitive::Long),
        _ => java::Type::named(path_of(written, ".")),
    }
}

/// The C# type node for a special type.
///
/// Four rows are C# keywords rather than names — `string` for both
/// string rows, `object` for `native`, `ulong` for `TimeBase::TimeT` and
/// `unsigned long long`, `decimal` for `fixed` and `long double` — and
/// they become [`csharp::Predefined`] rather than a one-segment path. A
/// `Named` node holding the text `string` renders identically today and
/// stops being a keyword the moment a generated file declares a class of
/// that name.
#[must_use]
pub fn csharp_type(t: SpecialType) -> csharp::Type {
    match language_mapping(t, TargetLanguage::CSharp) {
        "string" => csharp::Type::Predefined(csharp::Predefined::String),
        "object" => csharp::Type::Predefined(csharp::Predefined::Object),
        "ulong" => csharp::Type::Predefined(csharp::Predefined::ULong),
        "decimal" => csharp::Type::Predefined(csharp::Predefined::Decimal),
        written => csharp::Type::named(path_of(written, ".")),
    }
}

/// The Rust type node for a special type.
#[must_use]
pub fn rust_type(t: SpecialType) -> rust::Type {
    match t {
        SpecialType::SequenceOfAny => rust::Type::generic(
            Path::single("Vec"),
            alloc::vec![rust_type(SpecialType::Any)],
        ),
        SpecialType::ULongLong | SpecialType::Time => rust::Type::Primitive(rust::Primitive::U64),
        SpecialType::String => rust::Type::named(Path::single("String")),
        _ => rust::Type::named(path_of(language_mapping(t, TargetLanguage::Rust), "::")),
    }
}

/// Split a written name into a [`Path`] on the language's separator.
fn path_of(written: &str, separator: &str) -> Path {
    let segments: Vec<String> = written
        .split(separator)
        .map(alloc::string::ToString::to_string)
        .collect();
    let borrowed: Vec<&str> = segments.iter().map(String::as_str).collect();
    Path::new(&borrowed)
}

/// Every row, as a table a document or a test can walk.
///
/// Returns one line per special type: its IDL spelling, then the four
/// columns in [`TargetLanguage::ALL`] order.
#[must_use]
pub fn mapping_table() -> Vec<(SpecialType, [&'static str; 4])> {
    all_special_types()
        .into_iter()
        .map(|t| {
            (
                t,
                [
                    language_mapping(t, TargetLanguage::Cpp),
                    language_mapping(t, TargetLanguage::CSharp),
                    language_mapping(t, TargetLanguage::Java),
                    language_mapping(t, TargetLanguage::Rust),
                ],
            )
        })
        .collect()
}

/// The table as a Markdown fragment, so a document and this crate cannot
/// drift apart.
#[must_use]
pub fn mapping_table_markdown() -> String {
    let mut out = String::from("| IDL | C++ | C# | Java | Rust |\n|---|---|---|---|---|\n");
    for (t, columns) in mapping_table() {
        out.push_str("| `");
        out.push_str(t.idl_spelling());
        out.push('`');
        for column in columns {
            out.push_str(" | `");
            out.push_str(column);
            out.push('`');
        }
        out.push_str(" |\n");
    }
    out
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    #[test]
    fn exactly_thirteen_special_types() {
        assert_eq!(all_special_types().len(), 13);
        let mut seen = Vec::new();
        for t in all_special_types() {
            assert!(!seen.contains(&t), "{t:?} listed twice");
            seen.push(t);
        }
    }

    #[test]
    fn every_row_answers_in_every_column() {
        for t in all_special_types() {
            for lang in TargetLanguage::ALL {
                let mapped = language_mapping(t, lang);
                assert!(!mapped.is_empty(), "{t:?} has no {lang:?} mapping");
            }
        }
    }

    #[test]
    fn the_cst_language_sets_overlap_and_the_gaps_are_reported() {
        // C has a tree and no OMG CORBA PSM, and answers `None` rather
        // than the nearest neighbour. C# used to answer `None` in the
        // other direction, for want of a tree rather than of a mapping;
        // it has both now.
        assert_eq!(
            TargetLanguage::CSharp.cst_language(),
            Some(Language::CSharp)
        );
        assert_eq!(TargetLanguage::from_cst_language(Language::C), None);
        for lang in TargetLanguage::RENDERABLE {
            let cst = lang.cst_language().expect("renderable has a tree");
            assert_eq!(TargetLanguage::from_cst_language(cst), Some(lang));
        }
    }

    #[test]
    fn long_double_takes_a_real_binary128_in_rust() {
        // The row this rebuild changed. `f64` was the old answer and it
        // put eight bytes of a value nobody wrote on the wire.
        assert_eq!(
            language_mapping(SpecialType::LongDouble, TargetLanguage::Rust),
            "zerodds_float128::Float128"
        );
        assert_eq!(
            zerodds_lang_render::rust::type_(&rust_type(SpecialType::LongDouble)),
            "zerodds_float128::Float128"
        );
    }

    #[test]
    fn the_three_corba_runtime_rows_name_corba_rt() {
        for t in [
            SpecialType::Any,
            SpecialType::TypeCode,
            SpecialType::WString,
        ] {
            assert!(
                language_mapping(t, TargetLanguage::Rust).starts_with("zerodds_corba_rt::"),
                "{t:?} should be a corba-rt type"
            );
        }
    }

    #[test]
    fn cpp_native_renders_as_a_pointer_and_not_as_an_identifier() {
        let rendered = zerodds_lang_render::cpp::type_(&cpp_type(SpecialType::NativeRef));
        assert_eq!(rendered.replace(' ', ""), "void*");
    }

    #[test]
    fn cpp_rows_render_back_to_the_table() {
        for t in all_special_types() {
            if t == SpecialType::NativeRef {
                continue;
            }
            assert_eq!(
                zerodds_lang_render::cpp::type_(&cpp_type(t)),
                language_mapping(t, TargetLanguage::Cpp),
                "{t:?}"
            );
        }
    }

    #[test]
    fn java_rows_render_back_to_the_table() {
        for t in all_special_types() {
            assert_eq!(
                zerodds_lang_render::java::type_(&java_type(t)),
                language_mapping(t, TargetLanguage::Java),
                "{t:?}"
            );
        }
    }

    #[test]
    fn csharp_rows_render_back_to_the_table() {
        for t in all_special_types() {
            assert_eq!(
                zerodds_lang_render::csharp::type_(&csharp_type(t)),
                language_mapping(t, TargetLanguage::CSharp),
                "{t:?}"
            );
        }
    }

    #[test]
    fn the_four_csharp_keyword_rows_are_predefined_and_not_paths() {
        for t in [
            SpecialType::String,
            SpecialType::WString,
            SpecialType::NativeRef,
            SpecialType::Time,
            SpecialType::ULongLong,
            SpecialType::Fixed,
            SpecialType::LongDouble,
        ] {
            assert!(
                matches!(csharp_type(t), csharp::Type::Predefined(_)),
                "{t:?} should be a C# keyword type"
            );
        }
    }

    #[test]
    fn java_valuebase_is_serializable_not_a_corba_class() {
        assert_eq!(
            language_mapping(SpecialType::ValueBase, TargetLanguage::Java),
            "java.io.Serializable"
        );
    }

    #[test]
    fn a_scoped_name_answers_the_two_declared_rows() {
        assert_eq!(
            special_type_of_scoped(&["TimeBase", "TimeT"]),
            Some(SpecialType::Time)
        );
        assert_eq!(
            special_type_of_scoped(&["", "CORBA", "TypeCode"]),
            Some(SpecialType::TypeCode)
        );
        assert_eq!(special_type_of_scoped(&["MyModule", "TimeT"]), None);
    }

    #[test]
    fn the_markdown_table_has_a_row_for_each_special_type() {
        let table = mapping_table_markdown();
        assert_eq!(table.lines().count(), 2 + 13);
        for t in all_special_types() {
            assert!(
                table.contains(t.idl_spelling()),
                "{t:?} missing from the table"
            );
        }
    }

    #[test]
    fn each_idl_spelling_is_written_once() {
        let mut seen = Vec::new();
        for t in all_special_types() {
            let spelling = t.idl_spelling().to_string();
            assert!(!seen.contains(&spelling), "{spelling} listed twice");
            seen.push(spelling);
        }
    }
}
