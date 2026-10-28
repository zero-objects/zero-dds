// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The C# CST as the other end of a rule. Spec §3.2.
//!
//! The counterpart of [`crate::adapter::java`], which does the same for
//! Java, of [`crate::adapter::c`] for C and of [`crate::adapter::cst`]
//! for Rust. Reading two of them side by side is the point: where they
//! differ, the difference is the language's, and it is written down here
//! rather than spread through a renderer.
//!
//! Values are resolved from the model, never from the overlay — the same
//! rule the other materialisers follow and for the same reason
//! (spec §2.3).
//!
//! ## The four decisions this column makes
//!
//! ### 1. An IDL `struct` becomes a C# `class`, not a C# `struct`
//!
//! Three reasons, any one of which is enough:
//!
//! - **A `struct` is copied.** C# assigns and passes a `struct` by value.
//!   A codec handed a sample to fill writes into the copy, the caller
//!   keeps the empty original, and nothing reports anything. That is the
//!   silent-drop class this repository is arranged against, and it is a
//!   language property rather than a coding mistake anyone could avoid.
//! - **A `struct` cannot be `null`.** `@optional` is absence, and
//!   `null` is what absence is here (see below). A holder that has no
//!   `null` needs a second field to say "not present", and two fields
//!   that can disagree is exactly what the Java column refused for the
//!   same construct.
//! - **A `struct` may not contain itself.** CS0523 forbids an instance
//!   field whose type is the enclosing struct, directly or through
//!   another struct, and IDL reaches that shape through `@external` and
//!   through a boxed value.
//!
//! The cost is stated rather than hidden: a small aggregate is heap
//! allocated where a `struct` would not be. That is a throughput
//! question with a measurable answer; a value silently written into a
//! copy is not.
//!
//! `Float128` below is the one generated `struct`, because it is a
//! primitive: sixteen octets, never null, never recursive.
//!
//! ### 2. An IDL module becomes a `namespace`
//!
//! Not a nested class, which is what the Java column had to do. Two
//! Java rules forced that one: a file's public top-level type must be
//! named after the file, and a file may hold only one. C# has neither —
//! a compilation unit declares as many top-level types as it likes under
//! any file name — so the scope IDL declares is a scope here too, and
//! `module a { struct P; }` and `module b { struct P; }` are two types
//! with no outer class between them.
//!
//! Every reference to a generated type is written **rooted**:
//! `global::a.b.Point`. C# separates type lookup from expression lookup,
//! so a field named `a` cannot capture the type `a.b.Point` the way JLS
//! §6.5.2 lets it in Java — but a *nested* namespace can: inside
//! `namespace common.a`, the name `a` binds to `common.a` before the
//! global `a`. `global::` resolves at the global namespace and nothing
//! in any scope can capture it, which is why every path this
//! materialiser writes carries it.
//!
//! ### 3. Members are `PascalCase` fields carrying their IDL name
//!
//! **Fields, not properties.** A property cannot be passed as `ref` or
//! `out` (CS0206), and a decoder that fills a member in place needs
//! exactly that. `init`-only properties on a `record` cannot be filled
//! incrementally at all.
//!
//! **`PascalCase`, because .NET names public members that way** and
//! every C# consumer already in this repository —
//! `zerodds-examples/csharp-typed`, `endpoints/csharp` — writes them so.
//!
//! **Only the first letter moves.** `venue` becomes `Venue` and
//! `my_field` becomes `My_field`. A full `PascalCase` fold —
//! `my_field` to `MyField` — is not injective, and IDL 4.2 §7.2.3 forbids
//! two identifiers in one scope that differ only in case but permits
//! `my_field` beside `myField`; both would fold to one C# name and one of
//! the two members would disappear. Raising the first letter alone is
//! injective over any scope IDL allows.
//!
//! **The IDL spelling is carried, on every field, in
//! `[global::ZeroDds.Idl.IdlName("…")]`.** This is not decoration and
//! not a comment. XTypes 1.3 §7.3.1.2.1.1 computes a member's ID under
//! `@autoid(HASH)` and under `@hashid` from the **IDL member name** — the
//! first four octets of its MD5 — so a type that only knows `Venue`
//! cannot compute the ID for `venue`, and the member it writes lands
//! under a different ID than the peer's. The precedent is
//! `crates/dlrl-codegen/src/csharp.rs`, which carries `[DlrlKey("venue")]`
//! for the same reason one layer up: the DCPS topic names the key
//! `venue`.
//!
//! Type, module and enumerator names are **not** folded. They are
//! verbatim but for keyword escaping, because a type's name reaches the
//! wire — `TypeObject`'s `type_name`, and the topic type name a peer
//! matches on — and a renamed type is a type nothing matches.
//!
//! ### 4. `@optional` is `null`, and a value type takes `T?`
//!
//! One member, not a member beside a flag: a `bool _has_m` and an `m`
//! are two things that can disagree and nothing in the type system stops
//! them. A reference type is nullable already; a value type — every
//! numeric, `char`, `bool`, a generated `enum`, `Float128` — takes
//! `Nullable<T>`, written `T?`. No `#nullable enable` is emitted, so `?`
//! appears only where it is a distinct type and never as an annotation
//! the compiler would warn about outside an annotation context (CS8632).
//!
//! ## The integer mapping
//!
//! C# has unsigned integers, which is the largest single difference from
//! the Java column: every IDL width maps to a C# keyword of the same
//! width and the same signedness, nothing is held as a pattern, and
//! nothing has to be widened to stay a number.
//!
//! | IDL | C# | width |
//! |---|---|---|
//! | `boolean` | `bool` | |
//! | `char` | `char` | one UTF-16 code unit; the wire form is one octet |
//! | `wchar` | `char` | one UTF-16 code unit |
//! | `int8` | `sbyte` | 8, signed |
//! | `octet`, `uint8` | `byte` | 8, unsigned |
//! | `short` | `short` | 16, signed |
//! | `unsigned short` | `ushort` | 16, unsigned |
//! | `long` | **`int`** | 32, signed |
//! | `unsigned long` | `uint` | 32, unsigned |
//! | `long long` | **`long`** | 64, signed |
//! | `unsigned long long` | `ulong` | 64, unsigned |
//! | `float` | `float` | binary32 |
//! | `double` | `double` | binary64 |
//! | `long double` | `global::ZeroDds.Idl.Float128` | binary128 |
//!
//! The two rows in bold are the trap this mapping exists to avoid. IDL
//! `long` is **32 bits** and C# `long` is **64**; IDL `long long` is 64
//! and C# `int` is 32. A column that carried the spelling across would
//! compile and write four octets where the peer reads eight.
//!
//! `long double` has no C# type at all. `decimal` is not it — `decimal`
//! is a base-ten type with 28 to 29 significant digits and a scale, not
//! IEEE 754 binary128 — so a sixteen-octet holder is generated instead
//! and named for what it is.
//!
//! ## The shapes
//!
//! | IDL | C# |
//! |---|---|
//! | `string`, `wstring` | `string` |
//! | `fixed<P,S>` | `string`, holding the decimal |
//! | `sequence<T>`, `T[N]` | `T[]` |
//! | `T[N][M]` | `T[][]` |
//! | `map<K,V>` | `ZdEntry_k_v[]` — an array of a two-field class |
//! | `union` | a class with `_d` and one field per case |
//! | `enum` | a real `enum : int` |
//! | `bitmask` | `[Flags] enum : <holder>` |
//! | `bitset` | a class of one field per bitfield |
//! | `typedef` | `using Name = T;` |
//! | `native` | `using Name = T;`, from the facet or `object` |
//! | `const` | a `const` field of a `ZdConstants` class |
//! | `interface` | an `interface` of operation signatures |
//!
//! ### An enumeration is a real `enum`, which Java could not use
//!
//! The Java column maps an enumeration to `int` constants and gives two
//! reasons. Neither holds in C#:
//!
//! - A Java `enum` constant's identity is its ordinal. A C# `enum`
//!   member's identity is the integer it is given, so `@value(-1)` and a
//!   `@bit_bound(32)` value are members like any other.
//! - A value the reader's IDL does not list has no Java `enum` constant.
//!   In C# every value of the underlying type is a value of the enum
//!   type — `(Colour)99` is legal and holds — so an `@appendable` writer
//!   that added an enumerator sends something the field can carry.
//!
//! The underlying type is `int`, written out. XTypes 1.3 §7.4.3.5.3
//! makes an enumerated literal 32 bits wide, and a value outside those
//! 32 bits is refused by name rather than truncated.
//!
//! ### A bitmask is a `[Flags]` enum
//!
//! Typed rather than a bare integer, with the underlying type Table 33
//! of XTypes 1.3 §7.4.1.1.3.2 gives for its bound: `byte`, `ushort`,
//! `uint` or `ulong`. `[Flags]` is what makes `ToString` print the set
//! rather than a number, and it changes nothing about the octets.
//!
//! ### A `map<K,V>` is not a `Dictionary<K,V>`
//!
//! The order of the entries is a policy axis — `MapEntryOrder` — and
//! every `Dictionary` either hashes the order away or fixes one of its
//! own. An array of a generated entry class keeps the sample's order
//! where the sample put it, and the axis stays measurable. The entry
//! class is named from the **IDL** kinds of its halves, not the C# ones,
//! so that two maps whose C# spelling coincides and whose wire widths do
//! not stay two classes.
//!
//! ### `fixed<P,S>` is a `string` and not a `decimal`
//!
//! `decimal` carries 28 to 29 significant digits; IDL 4.2 §7.4.1.4.4
//! allows up to 31. Beyond that, the `FixedDigitCount::ValuePrecision`
//! axis exists to measure which digit count a peer writes into the BCD
//! run, and a holder that normalises has already decided the answer. The
//! Java column reaches the same place from `BigDecimal`.
//!
//! ### A typedef is a `using` alias
//!
//! C# 12 aliases any type, arrays and tuples included, so `typedef
//! sequence<long> Row` is `using Row = int[];` — a form Java has no
//! counterpart for, where the Java column can only leave a comment.
//!
//! The alias is declared for the reader; **uses take the resolved
//! type**. A `using` alias may not be built on another `using` alias in
//! the same scope, so a chain `typedef long A; typedef A B;` could not be
//! written as one, and resolving at each use keeps the two forms of
//! typedef from behaving differently.
//!
//! ## What is refused
//!
//! `any`, `ValueBase`, an object reference and an annotation type have no
//! C# data form here and are named as
//! [`MaterialiseError::NoLanguageForm`] rather than left out of the
//! class. A generator that leaves a member out emits a type missing a
//! field and reports success.

use crate::adapter::c::ordered_types;
use crate::adapter::constant;
use crate::adapter::cst::MaterialiseError;
use crate::adapter::interface;
use crate::apply::Applied;
use crate::verbatim;
use seesaw_tgg::ident::GhostId;
use zerodds_ast::{Ast, CharWidth, ModuleNode, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::{Id, NodeId};
use zerodds_lang_cst::common::{Doc, Ident, Literal, Path};
use zerodds_lang_cst::csharp;

/// The namespace the generated support declarations sit in.
///
/// Emitted into the file rather than taken from a runtime assembly: a
/// generated file is one `csc` argument, and a column whose output only
/// compiles against a package that is not written yet would be a claim
/// rather than a mapping.
pub const SUPPORT_NAMESPACE: &[&str] = &["ZeroDds", "Idl"];

/// The attribute that carries a member's IDL spelling.
const IDL_NAME_ATTRIBUTE: &str = "IdlName";

/// The generated holder for an IDL `long double`.
const FLOAT128: &str = "Float128";

/// The prefix a generated map-entry class carries.
const ENTRY_PREFIX: &str = "ZdEntry_";

/// The class a scope's IDL constants become members of.
///
/// C# has no namespace-level field, so a constant needs a type to live
/// in. One static class per scope rather than one per constant: the
/// names inside it are the IDL's own.
const CONSTANTS_CLASS: &str = "ZdConstants";

/// The names a generated declaration may not take at the global scope.
///
/// Every reference this materialiser writes is rooted at `global::`, and
/// `global::ZeroDds` or `global::System` would then resolve to a
/// generated namespace instead of the one meant. `global` itself is
/// listed because a namespace of that name makes every rooted path in
/// the file ambiguous to a reader even where the compiler copes.
const RESERVED_TOP_LEVEL: &[&str] = &["ZeroDds", "System", "global"];

/// How deep a module chain is followed before the model is taken to be
/// malformed. IDL nests nowhere near this.
const MAX_MODULE_DEPTH: usize = 64;

/// How deep a type's C# spelling is followed.
///
/// A `sequence<sequence<…>>` nests once per level; a chain longer than
/// this is a cycle the resolver did not catch, and recursing into it
/// would end the process rather than report anything.
const MAX_TYPE_DEPTH: usize = 64;

/// How many times a colliding member name is suffixed before the model
/// is taken to be malformed.
///
/// A member's C# name may have to move once — CS0542 forbids a member
/// sharing its enclosing type's name — and a second time if the moved
/// name has been taken too. A bound rather than a `loop`: an unbounded
/// rename would hang on a model that somehow produced them all.
const MAX_RENAME_ATTEMPTS: usize = 64;

/// Build the C# file a rule application produced.
///
/// The whole file and not a list of declarations: `using` directives
/// have to precede every declaration in their scope, so a signature that
/// returned only declarations could not express a typedef at all.
/// [`csharp::File::doc`] is left empty for the caller to fill.
///
/// # Errors
/// [`MaterialiseError`] when a value the output has to state does not
/// evaluate, a type has no C# shape, or a generated name would collide
/// with another in its scope — never a silently dropped declaration.
pub fn csharp_file(ast: &Ast, applied: &Applied) -> Result<csharp::File, MaterialiseError> {
    let mut root = Scope::default();

    // Structural declarations — the map entry classes — sit at the
    // global scope and not in a namespace: two identical maps declared
    // in two modules are one C# class, exactly as two identical
    // anonymous collections are one C type.
    let mut structural: Vec<csharp::Item> = Vec::new();
    let mut emitted: Vec<String> = Vec::new();

    // The same walk the C and Java columns use. C# needs no forward
    // declarations, so the order is not load-bearing for the compiler;
    // it is load-bearing for the output, which must not move between two
    // runs over one model.
    for id in ordered_types(ast) {
        let start = match ast.type_(id) {
            Ok(node) => root.at(ast, node.module)?.items.len(),
            Err(_) => 0,
        };
        declare(ast, applied, id, &mut root, &mut structural, &mut emitted)?;
        weave_verbatim(ast, id, start, &mut root)?;
    }

    // Constants and exceptions are not types, so they are walked
    // separately — the model keeps them in arenas of their own, and
    // every sibling materialiser does the same.
    constants(ast, applied, &mut root)?;
    exceptions(ast, applied, &mut root)?;
    interfaces(ast, applied, &mut root)?;

    // `@verbatim(placement = BEGIN_FILE)` opens the generated file and
    // `END_FILE` closes it — OMG IDL 4.2 §8.3.5.1 — before even the
    // support namespace, because the file and not any scope is what the
    // placement names.
    let (begin_file, end_file) = verbatim::file_slots(ast, verbatim::aliases::CSHARP);
    let mut items: Vec<csharp::Item> = begin_file.into_iter().map(csharp::Item::Verbatim).collect();
    items.push(csharp::Item::Namespace(support_namespace()));
    items.extend(structural);
    let (usings, scope_items) = scope_items(root, 0)?;
    items.extend(scope_items);
    items.extend(end_file.into_iter().map(csharp::Item::Verbatim));
    check_names(&items, 0)?;

    Ok(csharp::File {
        doc: Doc::none(),
        usings,
        items,
    })
}

/// Weave one type's `@verbatim` text around and into what `declare`
/// just pushed. OMG IDL 4.2 §8.3.5.1; the slot semantics are
/// [`crate::verbatim`]'s.
///
/// `start` is where the type's items begin in its module's scope.
/// `BEGIN_DECLARATION` and `END_DECLARATION` land inside the class or
/// struct body when the type materialised as one; an enum, a `using`
/// alias or a type that declared nothing has no body this tree can hold
/// raw text in, so the text lands adjacent to the declaration instead —
/// the nearest position the tree can express, never a dropped
/// directive.
///
/// # Errors
/// [`MaterialiseError::NoModule`] when the model does not answer for
/// the type's module, exactly as `declare` would have reported it.
fn weave_verbatim(
    ast: &Ast,
    id: Id<TypeNode>,
    start: usize,
    root: &mut Scope,
) -> Result<(), MaterialiseError> {
    let slots = verbatim::declaration_slots(ast, id, verbatim::aliases::CSHARP);
    if slots.before.is_empty()
        && slots.begin.is_empty()
        && slots.end.is_empty()
        && slots.after.is_empty()
    {
        return Ok(());
    }
    let Ok(node) = ast.type_(id) else {
        return Ok(());
    };
    let name = csharp::escape_keyword(&node.name);
    let scope = root.at(ast, node.module)?;
    let mut declared = scope.items.split_off(start);
    let mut begin = slots.begin;
    let mut end = slots.end;
    let body = declared.iter_mut().find_map(|item| match item {
        csharp::Item::Type(csharp::TypeDecl::Class(class)) if class.name.as_str() == name => {
            Some(&mut class.members)
        }
        csharp::Item::Type(csharp::TypeDecl::Struct(node)) if node.name.as_str() == name => {
            Some(&mut node.members)
        }
        _ => None,
    });
    if let Some(members) = body {
        for text in begin.drain(..).rev() {
            members.insert(0, csharp::Member::Verbatim(text));
        }
        members.extend(end.drain(..).map(csharp::Member::Verbatim));
    }
    scope.items.extend(
        slots
            .before
            .into_iter()
            .chain(begin)
            .map(csharp::Item::Verbatim),
    );
    scope.items.append(&mut declared);
    scope.items.extend(
        end.into_iter()
            .chain(slots.after)
            .map(csharp::Item::Verbatim),
    );
    Ok(())
}

// -- the support namespace ------------------------------------------------

/// `namespace ZeroDds.Idl { … }` — the attribute and the `long double`
/// holder every generated file carries.
fn support_namespace() -> csharp::Namespace {
    csharp::Namespace {
        doc: Doc::lines(&[
            "What the generated declarations below are written against.",
            "It is emitted into the file rather than referenced from an assembly:",
            "one generated file is one compiler argument, and a column that only",
            "built against a package nobody has written would be a claim.",
        ]),
        name: Path::new(SUPPORT_NAMESPACE),
        usings: Vec::new(),
        items: vec![
            csharp::Item::Type(csharp::TypeDecl::Class(idl_name_attribute())),
            csharp::Item::Type(csharp::TypeDecl::Struct(float128_struct())),
        ],
    }
}

/// The `IdlName` attribute declaration.
fn idl_name_attribute() -> csharp::Class {
    let name_property = csharp::Member::Property(csharp::Property {
        doc: Doc::line("The identifier as the IDL source spelled it."),
        attributes: Vec::new(),
        modifiers: vec![csharp::Modifier::Public],
        type_: csharp::Type::Predefined(csharp::Predefined::String),
        name: Ident::new("Name"),
        accessors: vec![csharp::Accessor {
            kind: csharp::AccessorKind::Get,
            modifiers: Vec::new(),
            body: None,
            expression: None,
        }],
        expression: None,
        initialiser: None,
    });
    let constructor = csharp::Member::Constructor(csharp::Constructor {
        doc: Doc::none(),
        attributes: Vec::new(),
        modifiers: vec![csharp::Modifier::Public],
        name: Ident::new(&idl_name_class()),
        params: vec![csharp::Param {
            attributes: Vec::new(),
            modifier: None,
            type_: csharp::Type::Predefined(csharp::Predefined::String),
            name: Ident::new("name"),
            default: None,
        }],
        initialiser: None,
        body: csharp::Block {
            statements: vec![csharp::Statement::Expr(csharp::Expr::Assign {
                target: Box::new(csharp::Expr::Path(Path::single("Name"))),
                value: Box::new(csharp::Expr::Path(Path::single("name"))),
            })],
        },
    });
    csharp::Class {
        doc: Doc::lines(&[
            "The IDL spelling of the member it annotates.",
            "",
            "Load-bearing, not documentation. A member's ID under @autoid(HASH)",
            "and under @hashid is the first four octets of the MD5 of its IDL",
            "name (XTypes 1.3 7.3.1.2.1.1), so a field the mapping renamed to",
            "PascalCase cannot compute its own ID without this.",
        ]),
        attributes: Vec::new(),
        modifiers: vec![csharp::Modifier::Public, csharp::Modifier::Sealed],
        name: Ident::new(&idl_name_class()),
        generics: Vec::new(),
        bases: vec![csharp::Type::named(Path::rooted(&["System", "Attribute"]))],
        constraints: Vec::new(),
        members: vec![constructor, name_property],
    }
}

/// The attribute class's name; C# lets a use drop the `Attribute` suffix.
fn idl_name_class() -> String {
    format!("{IDL_NAME_ATTRIBUTE}Attribute")
}

/// The `long double` holder.
fn float128_struct() -> csharp::Struct {
    let field = |name: &str, doc: Doc| {
        csharp::Member::Field(csharp::Field {
            doc,
            attributes: Vec::new(),
            modifiers: vec![csharp::Modifier::Public, csharp::Modifier::Readonly],
            type_: csharp::Type::Predefined(csharp::Predefined::ULong),
            name: Ident::new(name),
            value: None,
        })
    };
    let constructor = csharp::Member::Constructor(csharp::Constructor {
        doc: Doc::none(),
        attributes: Vec::new(),
        modifiers: vec![csharp::Modifier::Public],
        name: Ident::new(FLOAT128),
        params: vec![
            csharp::Param {
                attributes: Vec::new(),
                modifier: None,
                type_: csharp::Type::Predefined(csharp::Predefined::ULong),
                name: Ident::new("low"),
                default: None,
            },
            csharp::Param {
                attributes: Vec::new(),
                modifier: None,
                type_: csharp::Type::Predefined(csharp::Predefined::ULong),
                name: Ident::new("high"),
                default: None,
            },
        ],
        initialiser: None,
        body: csharp::Block {
            statements: vec![
                csharp::Statement::Expr(csharp::Expr::Assign {
                    target: Box::new(csharp::Expr::Path(Path::single("Low"))),
                    value: Box::new(csharp::Expr::Path(Path::single("low"))),
                }),
                csharp::Statement::Expr(csharp::Expr::Assign {
                    target: Box::new(csharp::Expr::Path(Path::single("High"))),
                    value: Box::new(csharp::Expr::Path(Path::single("high"))),
                }),
            ],
        },
    });
    csharp::Struct {
        doc: Doc::lines(&[
            "IEEE 754 binary128 - the sixteen octets CDR gives a `long double`.",
            "",
            ".NET has no 128-bit floating type. `decimal` is not one: it is a",
            "base-ten type with 28 to 29 significant digits and a scale, and a",
            "member typed against it would put a different number on the wire",
            "than the IDL wrote. What is held here is the pattern - one sign",
            "bit, a fifteen-bit exponent biased by 16383 and a 112-bit stored",
            "fraction (CORBA 3.3 part 2 9.3.1.3, XTypes 1.3 Table 31) - split",
            "into its low and high halves.",
            "",
            "A `struct` where every other generated aggregate is a `class`,",
            "because this is a primitive: fixed at sixteen octets, never absent",
            "and never recursive. Arithmetic is not here; the counterparts in",
            "crates/float128 and runtime/java hold the same pattern.",
        ]),
        attributes: Vec::new(),
        modifiers: vec![csharp::Modifier::Public, csharp::Modifier::Readonly],
        name: Ident::new(FLOAT128),
        generics: Vec::new(),
        bases: Vec::new(),
        constraints: Vec::new(),
        members: vec![
            field("Low", Doc::line("The low 64 bits of the pattern.")),
            field(
                "High",
                Doc::line("The high 64 bits: sign, exponent, top fraction."),
            ),
            constructor,
        ],
    }
}

// -- the scope tree -------------------------------------------------------

/// One nesting level of the output: a module's declarations, the `using`
/// aliases it carries, and the modules declared inside it.
///
/// A tree and not a list of chains. C# lets a namespace be reopened, so
/// two blocks of one name would compile — and would put
/// `module common { struct Robot; }` and
/// `module common { module util { … } }` in two places a reader has to
/// join up. One block per module, in first-seen order, is what makes two
/// runs over one model produce one file.
#[derive(Default)]
struct Scope {
    /// `using` aliases declared in this scope, in declaration order.
    usings: Vec<csharp::Using>,
    /// What this module declares directly, in declaration order.
    items: Vec<csharp::Item>,
    /// The constants declared in it, which become one class.
    constants: Vec<csharp::Member>,
    /// The modules declared inside it, in first-seen order.
    children: Vec<(Id<ModuleNode>, String, Scope)>,
}

impl Scope {
    /// The scope a module names, creating it on the way down.
    ///
    /// # Errors
    /// [`MaterialiseError::NoModule`] when the model does not answer for
    /// a module in the chain.
    fn at(&mut self, ast: &Ast, module: Id<ModuleNode>) -> Result<&mut Self, MaterialiseError> {
        let chain = module_chain(ast, module)?;
        let mut scope = self;
        for (id, name) in chain {
            let at = match scope.children.iter().position(|(seen, _, _)| *seen == id) {
                Some(at) => at,
                None => {
                    scope.children.push((id, name, Self::default()));
                    scope.children.len() - 1
                }
            };
            scope = &mut scope
                .children
                .get_mut(at)
                .ok_or_else(|| MaterialiseError::NoModule {
                    at: module_handle(module),
                })?
                .2;
        }
        Ok(scope)
    }
}

/// A scope's `using` directives and items, with each module inside it as
/// a nested namespace.
///
/// # Errors
/// [`MaterialiseError::NoModule`] when the nesting runs past
/// [`MAX_MODULE_DEPTH`], which [`Scope::at`] has already bounded.
///
/// zerodds-lint: recursion-depth 64 (module nesting; bounded by `MAX_MODULE_DEPTH`)
fn scope_items(
    scope: Scope,
    depth: usize,
) -> Result<(Vec<csharp::Using>, Vec<csharp::Item>), MaterialiseError> {
    if depth > MAX_MODULE_DEPTH {
        return Err(MaterialiseError::NoModule {
            at: format!("the nesting runs past {MAX_MODULE_DEPTH}"),
        });
    }
    let mut items = scope.items;
    if !scope.constants.is_empty() {
        items.push(csharp::Item::Type(csharp::TypeDecl::Class(csharp::Class {
            doc: Doc::line(
                "The constants this scope declares. C# has no namespace-level \
                 field, so a constant needs a type to sit in; the names inside \
                 are the IDL's own.",
            ),
            attributes: Vec::new(),
            modifiers: vec![csharp::Modifier::Public, csharp::Modifier::Static],
            name: Ident::new(CONSTANTS_CLASS),
            generics: Vec::new(),
            bases: Vec::new(),
            constraints: Vec::new(),
            members: scope.constants,
        })));
    }
    for (_, name, child) in scope.children {
        let (usings, nested) = scope_items(child, depth + 1)?;
        items.push(csharp::Item::Namespace(csharp::Namespace {
            doc: Doc::line("An IDL module, as the namespace its declarations sit in."),
            name: Path::single(&name),
            usings,
            items: nested,
        }));
    }
    Ok((scope.usings, items))
}

/// The modules a declaration nests in, outermost first, with the
/// namespace name each takes.
///
/// The root module is the anonymous global scope and is not a namespace.
///
/// # Errors
/// [`MaterialiseError::NoModule`] when the model does not answer for a
/// module, or the parent chain runs past [`MAX_MODULE_DEPTH`]. Returning
/// the declaration unnested instead would put it in another scope and
/// say nothing, which is the same class of substitution as writing the
/// wrong octet.
fn module_chain(
    ast: &Ast,
    module: Id<ModuleNode>,
) -> Result<Vec<(Id<ModuleNode>, String)>, MaterialiseError> {
    let mut out = Vec::new();
    let mut current = Some(module);
    let mut depth = 0;
    while let Some(id) = current {
        if depth > MAX_MODULE_DEPTH {
            return Err(MaterialiseError::NoModule {
                at: format!(
                    "{} — the parent chain runs past {MAX_MODULE_DEPTH}",
                    module_handle(module)
                ),
            });
        }
        let node = ast.module(id).map_err(|_| MaterialiseError::NoModule {
            at: module_handle(id),
        })?;
        // The root has no name and no parent, and produces no namespace.
        if node.parent.is_none() {
            break;
        }
        out.push((id, csharp::escape_keyword(&node.name)));
        current = node.parent;
        depth += 1;
    }
    out.reverse();
    Ok(out)
}

/// The module handle, named without the node the model would not answer
/// for. Slot and generation are what the arena was asked for.
fn module_handle(module: Id<ModuleNode>) -> String {
    format!("#{}@{}", module.slot(), module.generation())
}

/// Refuse a declaration whose name a C# compiler would reject.
///
/// Two collisions, both of which produce a file no compiler reads: two
/// declarations of one name in one scope (CS0101), and a global-scope
/// name that would capture the first segment of every rooted reference
/// this file writes.
///
/// zerodds-lint: recursion-depth 64 (namespace nesting; bounded by `MAX_MODULE_DEPTH`)
fn check_names(items: &[csharp::Item], depth: usize) -> Result<(), MaterialiseError> {
    if depth > MAX_MODULE_DEPTH {
        return Err(MaterialiseError::NoModule {
            at: format!("the nesting runs past {MAX_MODULE_DEPTH}"),
        });
    }
    let mut seen: Vec<String> = Vec::new();
    for item in items {
        let name = match item {
            csharp::Item::Namespace(node) => node
                .name
                .last()
                .map(|last| last.as_str().to_string())
                .unwrap_or_default(),
            csharp::Item::Type(node) => node.name().as_str().to_string(),
            csharp::Item::Comment(_) | csharp::Item::Verbatim(_) => continue,
        };
        if depth == 0 && RESERVED_TOP_LEVEL.contains(&name.as_str()) {
            return Err(MaterialiseError::NoLanguageForm {
                construct: format!(
                    "a declaration named `{name}` at the global scope, which would \
                     capture the first segment of every `global::` reference this \
                     file writes"
                ),
                language: "C#",
                at: name,
            });
        }
        if seen.contains(&name) {
            return Err(MaterialiseError::NoLanguageForm {
                construct: format!("a second declaration named `{name}` in one scope"),
                language: "C#",
                at: name,
            });
        }
        seen.push(name);
        if let csharp::Item::Namespace(node) = item {
            check_names(&node.items, depth + 1)?;
        }
    }
    Ok(())
}

// -- the declarations -----------------------------------------------------

/// One type's C# declarations, appended where they belong.
#[allow(clippy::too_many_lines)]
fn declare(
    ast: &Ast,
    applied: &Applied,
    id: Id<TypeNode>,
    root: &mut Scope,
    structural: &mut Vec<csharp::Item>,
    emitted: &mut Vec<String>,
) -> Result<(), MaterialiseError> {
    let Ok(node) = ast.type_(id) else {
        return Ok(());
    };
    match &node.kind {
        // A primitive, a string and a `fixed` are spellings C# already
        // has; a sequence is `T[]`. None of them declares anything.
        TypeKind::Primitive(_)
        | TypeKind::String { .. }
        | TypeKind::Fixed { .. }
        | TypeKind::Sequence { .. } => {}

        // Neither does an array — but every dimension is evaluated
        // anyway. A C# array type does not state a length, so nothing
        // here would substitute a zero; what a dimension that does not
        // evaluate means is that no caller can allocate the array, and a
        // file that compiles while the length is unknown is exactly the
        // silent success this project refuses. The C and Java columns
        // fail on the same input, and the three columns' failure sets
        // stay comparable.
        TypeKind::Array { dims, .. } => {
            for dim in dims {
                ast.eval_integral(*dim)
                    .map_err(|_| MaterialiseError::Unevaluated {
                        construct: "array length".to_string(),
                        at: named(&node.name, node.node_id),
                    })?;
            }
        }

        TypeKind::Map { key, value, .. } => {
            let (Some(key), Some(value)) = (key.resolved(), value.resolved()) else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "map key or value".to_string(),
                    at: named(&node.name, node.node_id),
                });
            };
            let name = map_entry_name(ast, id)?;
            if emitted.contains(&name) {
                return Ok(());
            }
            emitted.push(name.clone());
            let at = named(&node.name, node.node_id);
            let members = fields_of(
                &name,
                vec![
                    PendingField {
                        idl_name: "key".to_string(),
                        type_: csharp_member_type(ast, key, &at)?,
                        doc: Doc::none(),
                    },
                    PendingField {
                        idl_name: "value".to_string(),
                        type_: csharp_member_type(ast, value, &at)?,
                        doc: Doc::none(),
                    },
                ],
            )?;
            structural.push(csharp::Item::Type(csharp::TypeDecl::Class(data_class(
                &name,
                members,
                Doc::line(
                    "One entry of a map. XTypes 1.3 7.4.1.1.4 defines a map as a \
                     sequence of these, and the rules write the two halves with no \
                     framing between them. An array of these and not a \
                     Dictionary<K,V>: the entry order is the sample's, and it is \
                     what the MapEntryOrder axis measures.",
                ),
            ))));
        }

        // A typedef becomes a `using` alias — the form C# has and Java
        // does not. The alias is for the reader; every use takes the
        // resolved type, because a `using` alias may not be built on
        // another one in the same scope.
        TypeKind::Alias { aliased } => {
            if node.name.is_empty() {
                return Ok(());
            }
            if ghost_created_for(applied, node.node_id, "CSharpAlias").is_none() {
                return Ok(());
            }
            let Some(target) = aliased.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "typedef target".to_string(),
                    at: node.name.clone(),
                });
            };
            let type_ = csharp_member_type(ast, target, &node.name)?;
            root.at(ast, node.module)?.usings.push(csharp::Using {
                doc: Doc::none(),
                alias: Some(Ident::new(&csharp::escape_keyword(&node.name))),
                static_: false,
                global: false,
                target: type_,
            });
        }

        // A native type is opaque: the mapping is the facet's spelling
        // and never a guess at what the name might mean. It takes an
        // alias for the same reason a typedef does.
        TypeKind::Native => {
            if ghost_created_for(applied, node.node_id, "CSharpNative").is_none() {
                return Ok(());
            }
            if node.name.is_empty() {
                return Ok(());
            }
            root.at(ast, node.module)?.usings.push(csharp::Using {
                doc: Doc::line("An IDL `native` type: opaque, mapped by its facet."),
                alias: Some(Ident::new(&csharp::escape_keyword(&node.name))),
                static_: false,
                global: false,
                target: native_type(ast, id),
            });
        }

        TypeKind::Enumeration { enumerators } => {
            if ghost_created_for(applied, node.node_id, "CSharpEnum").is_none() {
                return Ok(());
            }
            let mut members = Vec::new();
            let mut taken = vec![csharp::escape_keyword(&node.name)];
            for enumerator in enumerators {
                let Ok(enum_node) = ast.enumerator(*enumerator) else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "enumerator the model listed".to_string(),
                        at: node.name.clone(),
                    });
                };
                // Loud, where the Java column skips. Every rule in the
                // catalogue is unconditional — `part_of` carries no
                // predicate — so a part the rules did not create is a
                // defect in the engine or the catalogue and never a
                // mapping decision. Skipping it drops an enumerator from
                // the C# enum and leaves a file that still compiles.
                if ghost_created_for(applied, enum_node.node_id, "CSharpEnumMember").is_none() {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "enumerator, which no rule created a member for".to_string(),
                        at: enum_node.name.clone(),
                    });
                }
                let value = ast.effective_enumerator_value(*enumerator).ok_or_else(|| {
                    MaterialiseError::Unevaluated {
                        construct: "enumerator value".to_string(),
                        at: enum_node.name.clone(),
                    }
                })?;
                let value = in_range(
                    value,
                    i128::from(i32::MIN),
                    i128::from(u32::MAX),
                    "int",
                    &enum_node.name,
                )?;
                // A value above `int`'s range is written as the pattern
                // it will occupy: XTypes 1.3 7.4.3.5.3 makes an
                // enumerated literal 32 bits wide and a `@bit_bound(32)`
                // enumeration may use all of them. `in_range` above has
                // already refused anything outside those 32 bits.
                let literal = Literal::int(match i32::try_from(value) {
                    Ok(fits) => i128::from(fits),
                    Err(_) => value - (i128::from(u32::MAX) + 1),
                });
                let name = unique(&csharp::escape_keyword(&enum_node.name), &mut taken)?;
                members.push(csharp::EnumMember {
                    doc: Doc::none(),
                    attributes: vec![idl_name(&enum_node.name)],
                    name: Ident::new(&name),
                    value: Some(csharp::Expr::Literal(literal)),
                });
            }
            root.at(ast, node.module)?
                .items
                .push(csharp::Item::Type(csharp::TypeDecl::Enum(csharp::Enum {
                    doc: Doc::line(
                        "An enumeration. A real C# enum and not a set of int \
                         constants: a member's identity here is the integer it is \
                         given, so @value(-1) holds, and every value of the \
                         underlying type is a value of this type, so an \
                         @appendable writer that added an enumerator sends \
                         something a field of this type can carry.",
                    ),
                    attributes: vec![idl_name(&node.name)],
                    modifiers: vec![csharp::Modifier::Public],
                    name: Ident::new(&csharp::escape_keyword(&node.name)),
                    // Written out rather than left to the language
                    // default: XTypes 1.3 7.4.3.5.3 makes the literal 32
                    // bits, and a width that reaches the wire should be
                    // stated where a reader can check it.
                    underlying: Some(csharp::Predefined::Int),
                    members,
                })));
        }

        TypeKind::Bitmask { bits } => {
            if ghost_created_for(applied, node.node_id, "CSharpBitmask").is_none() {
                return Ok(());
            }
            let bound = ast.effective_bit_bound(id);
            let holder = unsigned_holder(bound);
            let mut members = Vec::new();
            let mut taken = vec![csharp::escape_keyword(&node.name)];
            for bit in bits {
                let Ok(bit_node) = ast.bit_value(*bit) else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "bitmask bit the model listed".to_string(),
                        at: node.name.clone(),
                    });
                };
                // Loud, for the reason the enumerator above gives: a
                // bit the rules did not create a member for would vanish
                // from the flags enum and the file would still compile.
                if ghost_created_for(applied, bit_node.node_id, "CSharpBitMember").is_none() {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "bitmask bit, which no rule created a member for".to_string(),
                        at: bit_node.name.clone(),
                    });
                }
                let position = ast.effective_bit_position(*bit).ok_or_else(|| {
                    MaterialiseError::Unevaluated {
                        construct: "bit position".to_string(),
                        at: bit_node.name.clone(),
                    }
                })?;
                // The value and not a `1 << n` expression. A shift is
                // typed by its left operand in C#, so `1 << 40` is an
                // int shift evaluating to 256 — silently — and writing
                // the suffix that avoids it differs per holder. The
                // decimal has one meaning under every holder, and the
                // position is in the comment beside it.
                let value = 1_i128.checked_shl(u32::from(position)).ok_or_else(|| {
                    MaterialiseError::NoLanguageForm {
                        construct: format!("a bit at position {position}"),
                        language: "C#",
                        at: bit_node.name.clone(),
                    }
                })?;
                let name = unique(&csharp::escape_keyword(&bit_node.name), &mut taken)?;
                members.push(csharp::EnumMember {
                    doc: Doc::line(&format!("Bit {position}.")),
                    attributes: vec![idl_name(&bit_node.name)],
                    name: Ident::new(&name),
                    value: Some(csharp::Expr::Literal(Literal::int(value))),
                });
            }
            root.at(ast, node.module)?
                .items
                .push(csharp::Item::Type(csharp::TypeDecl::Enum(csharp::Enum {
                    doc: Doc::line(
                        "A bitmask, as the flags enum its bits name. The underlying \
                         type is what Table 33 of XTypes 1.3 7.4.1.1.3.2 gives for \
                         its bound; [Flags] changes how ToString prints a value and \
                         nothing about the octets.",
                    ),
                    attributes: vec![
                        csharp::Attribute {
                            target: None,
                            path: Path::rooted(&["System", "Flags"]),
                            args: Vec::new(),
                        },
                        idl_name(&node.name),
                    ],
                    modifiers: vec![csharp::Modifier::Public],
                    name: Ident::new(&csharp::escape_keyword(&node.name)),
                    underlying: Some(holder),
                    members,
                })));
        }

        TypeKind::Bitset { .. } => {
            if ghost_created_for(applied, node.node_id, "CSharpBitset").is_none() {
                return Ok(());
            }
            let name = csharp::escape_keyword(&node.name);
            let mut pending = Vec::new();
            // The base's bitfields are fields of the derived class too:
            // an inherited bitset is one flat run.
            for bitfield in &ast.all_bitfields_of(id) {
                let Ok(field_node) = ast.bitfield(*bitfield) else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "bitset field the model listed".to_string(),
                        at: node.name.clone(),
                    });
                };
                // An anonymous field is padding: it moves the next field
                // along and carries no value of its own.
                let Some(field_name) = field_node.name.as_ref() else {
                    continue;
                };
                // The width's holder, not the declared one:
                // `bitfield<1, boolean>` names a range and not a wire
                // form, and the C and Java columns take the width for
                // the same reason. The packing into the anchor is the
                // codec's.
                pending.push(PendingField {
                    idl_name: field_name.clone(),
                    type_: csharp::Type::Predefined(unsigned_holder(field_node.bits)),
                    doc: Doc::none(),
                });
            }
            let members = fields_of(&name, pending)?;
            root.at(ast, node.module)?
                .items
                .push(csharp::Item::Type(csharp::TypeDecl::Class(data_class(
                    &name,
                    members,
                    Doc::line(
                        "A bitset. The fields are separate integers here; the \
                         packing into the anchor is the codec's.",
                    ),
                ))));
        }

        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let created = if matches!(node.kind, TypeKind::Valuetype { .. }) {
                "CSharpValue"
            } else {
                "CSharpClass"
            };
            if ghost_created_for(applied, node.node_id, created).is_none() {
                return Ok(());
            }
            let name = csharp::escape_keyword(&node.name);
            // IDL4 to C# 1.0 §7.14.1: "An IDL struct that inherits from a base IDL struct,
            // shall be declared as a C# public class that extends the class resulting from
            // mapping the base IDL struct." The class holds its own members; the wire still
            // carries the base's first (XTypes 1.3 §7.4.1.1.5.1), and the codec reaches them
            // through the base class.
            //
            // Every member is emitted, with no per-member rule lookup: a
            // member whose lookup missed would vanish from the class and
            // the file would still compile. The C and Java columns take
            // the same view.
            let mut pending = Vec::new();
            for member in ast.members_of(id).iter().copied() {
                let Ok(member_node) = ast.member(member) else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "member the model listed".to_string(),
                        at: node.name.clone(),
                    });
                };
                let Some(target) = member_node.type_.resolved() else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "member type".to_string(),
                        at: member_node.name.clone(),
                    });
                };
                let type_ = csharp_member_type(ast, target, &member_node.name)?;
                if member_node.facets.is_optional() {
                    pending.push(PendingField {
                        idl_name: member_node.name.clone(),
                        type_: optional(ast, target, type_)?,
                        doc: Doc::line(
                            "An @optional member: null when absent. In a @mutable \
                             type an absent member is not written at all; outside \
                             one its absence is written - a zero is_present byte \
                             under XCDR2 (rule 20) or a parameter header declaring \
                             a length of zero under XCDR1 (7.4.1.2).",
                        ),
                    });
                } else {
                    pending.push(PendingField {
                        idl_name: member_node.name.clone(),
                        type_,
                        doc: Doc::none(),
                    });
                }
            }
            let members = fields_of(&name, pending)?;
            let mut class = data_class(&name, members, Doc::none());
            if let Some(base) = super::c::struct_base(ast, id) {
                class.bases.push(csharp_member_type(ast, base, &node.name)?);
            }
            // A class something derives from cannot be sealed.
            if super::java::has_derived(ast, id) {
                class
                    .modifiers
                    .retain(|modifier| *modifier != csharp::Modifier::Sealed);
            }
            root.at(ast, node.module)?
                .items
                .push(csharp::Item::Type(csharp::TypeDecl::Class(class)));
        }

        TypeKind::Union { discriminator, .. } => {
            if ghost_created_for(applied, node.node_id, "CSharpUnion").is_none() {
                return Ok(());
            }
            let Some(disc) = discriminator.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "union discriminator".to_string(),
                    at: node.name.clone(),
                });
            };
            let name = csharp::escape_keyword(&node.name);
            // A field per case and not one per type: two cases may carry
            // the same type, and a C# class has no untagged overlay to
            // put them in. Which field is live is what the discriminator
            // says.
            let mut pending = vec![PendingField {
                idl_name: "_d".to_string(),
                type_: csharp_member_type(ast, disc, &node.name)?,
                doc: Doc::line(
                    "The discriminator. Which case field is live is what this says. \
                     `_d` is the CORBA accessor's name and not an IDL member, so \
                     the IdlName below carries it rather than a name from the \
                     source.",
                ),
            }];
            for case in ast.cases_of(id) {
                let Ok(case_node) = ast.case(*case) else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "union case the model listed".to_string(),
                        at: node.name.clone(),
                    });
                };
                // Loud, for the reason the enumerator above gives: a
                // case the rules did not create a field for would vanish
                // from the class, and a decoder switching on the
                // discriminator would have nowhere to put the value.
                if ghost_created_for(applied, case_node.node_id, "CSharpCase").is_none() {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "union case, which no rule created a field for".to_string(),
                        at: node.name.clone(),
                    });
                }
                let Ok(member) = ast.member(case_node.member) else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "member behind a union case the model listed".to_string(),
                        at: node.name.clone(),
                    });
                };
                let Some(target) = member.type_.resolved() else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "union case type".to_string(),
                        at: member.name.clone(),
                    });
                };
                pending.push(PendingField {
                    idl_name: member.name.clone(),
                    type_: csharp_member_type(ast, target, &member.name)?,
                    doc: Doc::none(),
                });
            }
            let members = fields_of(&name, pending)?;
            root.at(ast, node.module)?
                .items
                .push(csharp::Item::Type(csharp::TypeDecl::Class(data_class(
                    &name,
                    members,
                    Doc::line("A union: the discriminator, and one field per case."),
                ))));
        }

        // A value box is a reference in C# already: the boxed type, null
        // when absent. Nothing to declare.
        TypeKind::ValueBox { .. } => {}

        // An object reference belongs to the RPC arc and an annotation
        // type has no runtime form at all — `zerodds_ast_layout` refuses
        // to plan either — so a C# declaration would be a shape nothing
        // could serialise. `any` and `ValueBase` are in the model because
        // Building Block Any is one of IDL 4.2's fifteen, and what they
        // hold is decided at run time. All four are named here rather
        // than passed over, and `csharp_member_type` refuses them by name
        // where a member would carry one.
        TypeKind::ObjectRef { .. }
        | TypeKind::AnnotationType { .. }
        | TypeKind::Any
        | TypeKind::ValueBase => {}
    }
    Ok(())
}

/// The IDL constants, as `const` fields of each scope's holder class.
///
/// # Errors
/// As [`csharp_file`].
fn constants(ast: &Ast, applied: &Applied, root: &mut Scope) -> Result<(), MaterialiseError> {
    for (id, node) in ast.constants() {
        if ghost_created_for(applied, node.node_id, "CSharpConst").is_none() {
            continue;
        }
        let _ = id;
        // `eval_const` and not `eval_integral`: `const string MODULE_ID`
        // evaluates perfectly well and is not a number, and asking the
        // integral question about it answered `Unevaluated` and dropped
        // the field.
        let value = ast
            .eval_const(node.expr)
            .map_err(|_| MaterialiseError::Unevaluated {
                construct: "constant's value".to_string(),
                at: node.name.clone(),
            })?;
        let type_ = match node.type_.resolved() {
            Some(target) => csharp_member_type(ast, target, &node.name)?,
            None => csharp::Type::Predefined(csharp::Predefined::Int),
        };
        let value = constant_expr(&type_, &value, &node.name)?;
        // The name is the IDL's, not folded: a constant is not a member,
        // nothing computes an ID from it, and SCREAMING_CASE is what IDL
        // constants are written in and what .NET leaves alone.
        let name = csharp::escape_keyword(&node.name);
        root.at(ast, node.module)?
            .constants
            .push(csharp::Member::Field(csharp::Field {
                doc: Doc::none(),
                attributes: vec![idl_name(&node.name)],
                modifiers: vec![csharp::Modifier::Public, csharp::Modifier::Const],
                type_,
                name: Ident::new(&name),
                value: Some(value),
            }));
    }
    Ok(())
}

/// The IDL exceptions, as data classes.
///
/// A holder and **not** a `System.Exception`. An IDL exception member
/// named `message`, `data` or `source` folds to `Message`, `Data` or
/// `Source`, which are `System.Exception`'s own members — the generated
/// field would hide an inherited one (CS0108) or fail to override a
/// property that has no setter, and nothing about the wire form decides
/// which of the two the peer meant. The Java column reaches the same
/// place from `Serializable` and `serialVersionUID`.
///
/// # Errors
/// As [`csharp_file`].
fn exceptions(ast: &Ast, applied: &Applied, root: &mut Scope) -> Result<(), MaterialiseError> {
    for (id, node) in ast.exceptions() {
        if ghost_created_for(applied, node.node_id, "CSharpError").is_none() {
            continue;
        }
        let _ = id;
        let name = csharp::escape_keyword(&node.name);
        let mut pending = Vec::new();
        for member in &node.members {
            let Ok(member_node) = ast.member(*member) else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "exception member the model listed".to_string(),
                    at: node.name.clone(),
                });
            };
            let Some(target) = member_node.type_.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "exception member type".to_string(),
                    at: member_node.name.clone(),
                });
            };
            pending.push(PendingField {
                idl_name: member_node.name.clone(),
                type_: csharp_member_type(ast, target, &member_node.name)?,
                doc: Doc::none(),
            });
        }
        let members = fields_of(&name, pending)?;
        root.at(ast, node.module)?
            .items
            .push(csharp::Item::Type(csharp::TypeDecl::Class(data_class(
                &name,
                members,
                Doc::lines(&[
                    "An exception, as the structure its members make.",
                    "",
                    "A data holder and not a System.Exception: an IDL member named",
                    "`message` or `data` folds onto one of System.Exception's own",
                    "members, and hiding an inherited member (CS0108) is not a",
                    "decision the wire form makes.",
                ]),
            ))));
    }
    Ok(())
}

/// One `adapter::interface::Parameter` as a C# `Param`.
fn csharp_param(
    ast: &Ast,
    param: &interface::Parameter,
) -> Result<csharp::Param, MaterialiseError> {
    Ok(csharp::Param {
        // No IdlName here. A parameter's name is not folded, so nothing
        // about it is at risk; an attribute that repeated the name it
        // already carries would state nothing.
        attributes: Vec::new(),
        modifier: match param.direction {
            zerodds_ast::ParamDirection::In => None,
            zerodds_ast::ParamDirection::Out => Some(csharp::ParamModifier::Out),
            zerodds_ast::ParamDirection::InOut => Some(csharp::ParamModifier::Ref),
        },
        type_: csharp_member_type(ast, param.type_, &param.name)?,
        // Verbatim: a parameter name is not a member, nothing computes
        // an ID from it, and IDL already writes parameters in the lower
        // camel case C# asks for.
        name: Ident::new(&csharp::escape_keyword(&param.name)),
        default: None,
    })
}

/// The IDL interfaces, as C# interfaces of operation signatures and
/// properties.
///
/// The shape the IDL declared and nothing else: the transport belongs to
/// the RPC arc, exactly as it does in the Rust column, whose
/// `interface-to-trait` produces a trait of signatures with no bodies.
/// Materialised rather than left to the rule alone — a catalogue rule
/// whose created node nothing reads is a rule claiming a coverage cell
/// that produces no output, and that is a claim rather than a mapping.
///
/// The name is verbatim and carries no `I` prefix. .NET convention asks
/// for one, and an IDL interface's name reaches the wire in its CORBA
/// repository ID — `IDL:Robot:1.0` — so `IRobot` would be a type nothing
/// resolves back.
///
/// A direction becomes the C# passing mode that means the same thing:
/// `out` is `out` and `inout` is `ref`. `in` is the default and is
/// written as nothing, which is what C# means by it.
///
/// An IDL `attribute` becomes a C# property — OMG's IDL-to-C# mapping's
/// own idiom for it, and the one shape here with no operation-pair
/// alternative to weigh against it. `readonly` writes a getter and no
/// setter; both directions' `getraises`/`setraises` are documented, not
/// enforced — C# has no checked exception, the same reason `raises` on
/// an operation is documented and not a `throws` clause.
///
/// A derived interface is flattened: the emitted interface has no base
/// list, so OMG IDL 4.2 §7.4.3's inherited operations and attributes are
/// written into it directly.
/// [`interface::all_operations_flattened`]/[`interface::
/// all_attributes_flattened`] are what answer with them, bases before
/// self, matching [`Ast::all_operations_of`]'s own order.
///
/// # Errors
/// As [`csharp_file`].
fn interfaces(ast: &Ast, applied: &Applied, root: &mut Scope) -> Result<(), MaterialiseError> {
    for (id, node) in ast.interfaces() {
        if ghost_created_for(applied, node.node_id, "CSharpInterface").is_none() {
            continue;
        }
        let name = csharp::escape_keyword(&node.name);
        let mut members = Vec::new();
        let mut taken = vec![name.clone()];

        let operations = interface::all_operations_flattened(ast, id)
            .map_err(|error| interface::interface_error_to_materialise(error, &node.name))?;
        for op in operations
            .iter()
            .filter(|op| op.origin == interface::Origin::Declared)
        {
            let mut params = Vec::new();
            for param in &op.params {
                params.push(csharp_param(ast, param)?);
            }
            let returns = match op.returns {
                None => csharp::Type::Predefined(csharp::Predefined::Void),
                Some(target) => csharp_member_type(ast, target, &op.name)?,
            };
            let method = unique(&csharp::escape_keyword(&capitalise(&op.name)), &mut taken)?;
            let doc = op
                .raises_line()
                .map_or_else(Doc::none, |line| Doc::line(&line));
            members.push(csharp::Member::Method(csharp::Method {
                doc,
                attributes: vec![idl_name(&op.name)],
                // No modifier: an interface member is public and
                // abstract by definition, and writing either is a
                // compile error before C# 8 and noise after it.
                modifiers: Vec::new(),
                returns,
                name: Ident::new(&method),
                generics: Vec::new(),
                params,
                constraints: Vec::new(),
                body: None,
                expression: None,
            }));
        }

        let attributes = interface::all_attributes_flattened(ast, id)
            .map_err(|error| interface::interface_error_to_materialise(error, &node.name))?;
        for attribute in &attributes {
            let type_ = csharp_member_type(ast, attribute.type_, &attribute.name)?;
            let property = unique(
                &csharp::escape_keyword(&capitalise(&attribute.name)),
                &mut taken,
            )?;
            let mut doc = Doc::none();
            if let Some(line) = attribute.get_raises_line() {
                doc.push(&format!("Getter: {line}"));
            }
            if let Some(line) = attribute.set_raises_line() {
                doc.push(&format!("Setter: {line}"));
            }
            let mut accessors = vec![csharp::Accessor {
                kind: csharp::AccessorKind::Get,
                modifiers: Vec::new(),
                body: None,
                expression: None,
            }];
            if !attribute.readonly {
                accessors.push(csharp::Accessor {
                    kind: csharp::AccessorKind::Set,
                    modifiers: Vec::new(),
                    body: None,
                    expression: None,
                });
            }
            members.push(csharp::Member::Property(csharp::Property {
                doc,
                attributes: vec![idl_name(&attribute.name)],
                modifiers: Vec::new(),
                type_,
                name: Ident::new(&property),
                accessors,
                expression: None,
                initialiser: None,
            }));
        }

        root.at(ast, node.module)?
            .items
            .push(csharp::Item::Type(csharp::TypeDecl::Interface(
                csharp::Interface {
                    doc: Doc::lines(&[
                        "An interface, as the operation signatures and attribute",
                        "properties it has.",
                        "",
                        "The shape and not the transport: how a call is carried is the RPC",
                        "arc's, and the Rust column's `interface-to-trait` stops in the same",
                        "place. `raises`/`getraises`/`setraises` have no C# clause - the",
                        "language has no checked exception - and are documented on each",
                        "member instead; `oneway` is a transport property, so it is not",
                        "written here either.",
                        "",
                        "A derived interface carries its bases' operations and attributes",
                        "too: this declaration has no base list, so IDL 4.2 7.4.3's",
                        "inherited members are written into it rather than reached through",
                        "one.",
                    ]),
                    attributes: vec![idl_name(&node.name)],
                    modifiers: vec![csharp::Modifier::Public],
                    name: Ident::new(&name),
                    generics: Vec::new(),
                    bases: Vec::new(),
                    constraints: Vec::new(),
                    members,
                },
            )));
    }
    Ok(())
}

// -- fields ---------------------------------------------------------------

/// One field, before its C# name has been settled against the others.
struct PendingField {
    /// The IDL spelling, which the attribute carries.
    idl_name: String,
    /// Its type.
    type_: csharp::Type,
    /// Its documentation.
    doc: Doc,
}

/// The members of a type, with every C# name settled.
///
/// The fold is `venue` to `Venue`; where that collides with the type's
/// own name — CS0542 forbids a member sharing it — or with a field
/// already placed, an underscore is appended until it does not. Neither
/// move loses anything: the `[IdlName]` attribute on every field carries
/// the IDL spelling verbatim, which is the only spelling a hashed member
/// ID can be computed from.
///
/// # Errors
/// [`MaterialiseError::NoLanguageForm`] when a name will not come apart
/// within [`MAX_RENAME_ATTEMPTS`].
fn fields_of(
    type_name: &str,
    pending: Vec<PendingField>,
) -> Result<Vec<csharp::Member>, MaterialiseError> {
    let mut taken = vec![type_name.to_string()];
    let mut out = Vec::new();
    for field in pending {
        let folded = field_name(&field.idl_name);
        let name = unique(&folded, &mut taken)?;
        out.push(csharp::Member::Field(csharp::Field {
            doc: field.doc,
            attributes: vec![idl_name(&field.idl_name)],
            modifiers: vec![csharp::Modifier::Public],
            type_: field.type_,
            name: Ident::new(&name),
            value: None,
        }));
    }
    Ok(out)
}

/// `name`, suffixed until it is not in `taken`, and recorded there.
///
/// # Errors
/// [`MaterialiseError::NoLanguageForm`] when [`MAX_RENAME_ATTEMPTS`] is
/// reached. A bound and not a `loop`: an unbounded rename would hang on
/// a model that somehow produced them all, and hanging says nothing.
fn unique(name: &str, taken: &mut Vec<String>) -> Result<String, MaterialiseError> {
    let mut candidate = name.to_string();
    for _ in 0..MAX_RENAME_ATTEMPTS {
        if !taken.contains(&candidate) {
            taken.push(candidate.clone());
            return Ok(candidate);
        }
        candidate.push('_');
    }
    Err(MaterialiseError::NoLanguageForm {
        construct: format!(
            "the name `{name}`, which will not come apart from the others in its \
             scope within {MAX_RENAME_ATTEMPTS} attempts"
        ),
        language: "C#",
        at: name.to_string(),
    })
}

/// The `[global::ZeroDds.Idl.IdlName("…")]` attribute.
fn idl_name(name: &str) -> csharp::Attribute {
    let mut segments: Vec<&str> = SUPPORT_NAMESPACE.to_vec();
    segments.push(IDL_NAME_ATTRIBUTE);
    csharp::Attribute {
        target: None,
        path: Path::rooted(&segments),
        args: vec![csharp::AttributeArg::Positional(csharp::Expr::Literal(
            Literal::string(name),
        ))],
    }
}

/// The C# name a member, a case or a bitfield takes from its IDL name.
///
/// The one place the fold is written, because two halves of the column
/// have to agree on it: this materialiser declares the field and
/// `crate::emit::csharp::CSharp::escape` reaches it from the codec. A
/// second reading would be a field no compiler finds — loud, but only
/// once a case exercises it.
///
/// What it does **not** carry is the collision rule. `fields_of`
/// suffixes a name that would take the enclosing type's (CS0542) or one
/// already placed, and it needs the other fields to know; the codec's
/// seam is handed a name and nothing else. The consequence is measured
/// rather than hidden: a member whose folded name collides produces a
/// codec that names the unsuffixed field, `csc` reports CS0117, and the
/// case is a refusal with the compiler's own message rather than a
/// silently wrong field.
#[must_use]
pub fn field_name(idl: &str) -> String {
    csharp::escape_keyword(&capitalise(idl))
}

/// Where a named declaration sits, rooted at the global namespace.
///
/// `global::a.b.Colour`. What a codec writes to reach an enumerator or a
/// bitmask constant the materialiser declared; the bare name resolves to
/// nothing outside the namespace it was declared in.
///
/// # Errors
/// [`MaterialiseError`] when the model will not answer for the type or
/// for a module it nests in.
pub fn declared_path(ast: &Ast, id: Id<TypeNode>) -> Result<String, MaterialiseError> {
    let path = qualified(ast, id)?;
    let mut out = String::from("global::");
    let mut first = true;
    for segment in path.segments() {
        if !first {
            out.push('.');
        }
        out.push_str(segment.as_str());
        first = false;
    }
    Ok(out)
}

/// Take the `ZeroDds.Idl` support namespace off a materialised file.
///
/// [`csharp_file`] emits `IdlNameAttribute` and `Float128` into every
/// file so that one generated source is one compiler argument. A caller
/// that compiles the file **with** `runtime/csharp` must not: that
/// assembly declares the same two names in the same namespace, and two
/// declarations of one name is CS0101.
///
/// The runtime's versions are a superset — the same two `ulong` fields
/// and the same constructor on `Float128`, plus the conversions the wire
/// path needs — and `runtime/csharp/tests/SelfTest.cs` holds the
/// two together so the superset claim is asserted rather than assumed.
///
/// Returns whether a namespace was removed, so a caller that expected
/// one and found none can say so instead of carrying on.
pub fn drop_support_namespace(file: &mut csharp::File) -> bool {
    let before = file.items.len();
    file.items.retain(|item| match item {
        csharp::Item::Namespace(node) => {
            node.name.segments().len() != SUPPORT_NAMESPACE.len()
                || !node
                    .name
                    .segments()
                    .iter()
                    .zip(SUPPORT_NAMESPACE)
                    .all(|(written, wanted)| written.as_str() == *wanted)
        }
        _ => true,
    });
    file.items.len() != before
}

/// `venue` to `Venue` — the first letter and nothing else.
///
/// The fold a full `PascalCase` would do is not injective: IDL 4.2
/// §7.2.3 forbids two identifiers in one scope that differ only in case
/// but permits `my_field` beside `myField`, and both fold to `MyField`.
/// One of the two members would then disappear from the class, which is
/// the failure this whole file is arranged against.
fn capitalise(name: &str) -> String {
    let mut chars = name.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().collect::<String>() + chars.as_str()
    })
}

/// `public sealed class <name> { … }`
///
/// `sealed`, because nothing derives from a generated data class: an
/// IDL base's members are folded into the derived type's own field list
/// (XTypes 1.3 §7.4.1.1.5.1), so a C# base class would hold a second
/// copy of them.
fn data_class(name: &str, members: Vec<csharp::Member>, doc: Doc) -> csharp::Class {
    csharp::Class {
        doc,
        attributes: vec![idl_name(name)],
        modifiers: vec![csharp::Modifier::Public, csharp::Modifier::Sealed],
        name: Ident::new(name),
        generics: Vec::new(),
        bases: Vec::new(),
        constraints: Vec::new(),
        members,
    }
}

// -- types ----------------------------------------------------------------

/// The C# type of a value in a position that has to hold one, refusing
/// the constructs C# cannot hold.
///
/// `any` is the first of those. What it holds is decided at run time —
/// there is no C# type for it and no wire form to generate a codec
/// against. Emitting `object a;` into the class and reporting success
/// produces a field no encoder can write, from a generator that said
/// nothing was wrong.
///
/// # Errors
/// [`MaterialiseError::NoLanguageForm`] for `any`, `ValueBase`, an
/// object reference and an annotation type;
/// [`MaterialiseError::Unevaluated`] for a type the model will not
/// answer for.
pub fn csharp_member_type(
    ast: &Ast,
    target: Id<TypeNode>,
    at: &str,
) -> Result<csharp::Type, MaterialiseError> {
    type_at(ast, target, at, 0)
}

/// [`csharp_member_type`], carrying how deep the spelling has nested.
///
/// zerodds-lint: recursion-depth 64 (collection and typedef nesting;
/// bounded by `MAX_TYPE_DEPTH`, which returns
/// `MaterialiseError::Unevaluated` naming the spelling that nested)
fn type_at(
    ast: &Ast,
    id: Id<TypeNode>,
    at: &str,
    depth: usize,
) -> Result<csharp::Type, MaterialiseError> {
    if depth > MAX_TYPE_DEPTH {
        return Err(MaterialiseError::Unevaluated {
            construct: format!("type, whose C# spelling nests past {MAX_TYPE_DEPTH}"),
            at: at.to_string(),
        });
    }
    let node = ast.type_(id).map_err(|_| MaterialiseError::Unevaluated {
        construct: "type".to_string(),
        at: at.to_string(),
    })?;
    let refused = match &node.kind {
        TypeKind::Any => Some("any"),
        TypeKind::ValueBase => Some("ValueBase"),
        // An object reference is a handle into the RPC arc and
        // `coverage.rs` says so; there is no C# data type for it.
        TypeKind::ObjectRef { .. } => Some("Object"),
        // An annotation type has no runtime form, so nothing declares
        // one and a reference to it would name a class that is not there.
        TypeKind::AnnotationType { .. } => Some("annotation type"),
        _ => None,
    };
    if let Some(construct) = refused {
        return Err(MaterialiseError::NoLanguageForm {
            construct: construct.to_string(),
            language: "C#",
            at: at.to_string(),
        });
    }
    Ok(match &node.kind {
        // Matched ahead of the arm below, which would reach
        // `csharp_primitive`'s `double` — eight octets where the member
        // is sixteen.
        TypeKind::Primitive(PrimitiveKind::Float128) => {
            let mut segments: Vec<&str> = SUPPORT_NAMESPACE.to_vec();
            segments.push(FLOAT128);
            csharp::Type::named(Path::rooted(&segments))
        }
        TypeKind::Primitive(kind) => csharp::Type::Predefined(csharp_primitive(*kind)),
        // A `wstring` is a `string` like a `string`: .NET strings are
        // UTF-16 already, and the surrogate rule of XTypes 1.3
        // 7.4.1.1.2 belongs to the codec.
        TypeKind::String { .. } | TypeKind::Fixed { .. } => {
            csharp::Type::Predefined(csharp::Predefined::String)
        }
        TypeKind::Sequence { element, .. } => {
            let Some(element) = element.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "sequence element".to_string(),
                    at: at.to_string(),
                });
            };
            csharp::Type::array(type_at(ast, element, at, depth + 1)?)
        }
        TypeKind::Array { element, dims } => {
            let Some(element) = element.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "array element".to_string(),
                    at: at.to_string(),
                });
            };
            let mut out = type_at(ast, element, at, depth + 1)?;
            // One `[]` per dimension, jagged. A C# rectangular `T[,]`
            // would state the rank and still not the lengths, and it
            // cannot be sliced into rows the way an encoder walks them;
            // the lengths are the plan's, and `declare` evaluates them
            // there.
            for _ in dims {
                out = csharp::Type::array(out);
            }
            out
        }
        TypeKind::Map { .. } => {
            csharp::Type::array(csharp::Type::named(Path::rooted(&[&map_entry_name(
                ast, id,
            )?])))
        }
        // A typedef is transparent: the use takes the target's type. The
        // alias `declare` emits is for the reader, and a `using` alias
        // may not be built on another one in the same scope.
        TypeKind::Alias { aliased } => {
            let Some(target) = aliased.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "typedef target".to_string(),
                    at: at.to_string(),
                });
            };
            type_at(ast, target, at, depth + 1)?
        }
        TypeKind::Structure { .. }
        | TypeKind::Union { .. }
        | TypeKind::Enumeration { .. }
        | TypeKind::Bitmask { .. }
        | TypeKind::Bitset { .. }
        | TypeKind::Valuetype { .. } => csharp::Type::named(qualified(ast, id)?),
        TypeKind::Native => native_type(ast, id),
        // A boxed value is a reference, absent when null — which is what
        // a C# class reference already is. A value type takes `T?`.
        TypeKind::ValueBox { boxed } => {
            let Some(target) = boxed.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "value box target".to_string(),
                    at: at.to_string(),
                });
            };
            let inner = type_at(ast, target, at, depth + 1)?;
            optional(ast, target, inner)?
        }
        TypeKind::Any
        | TypeKind::ValueBase
        | TypeKind::ObjectRef { .. }
        | TypeKind::AnnotationType { .. } => {
            // Refused above; the arm keeps the match total, so a kind
            // added to the model without a C# answer is a compile error
            // and not a field that silently takes another's type.
            return Err(MaterialiseError::NoLanguageForm {
                construct: node.kind.as_str().to_string(),
                language: "C#",
                at: at.to_string(),
            });
        }
    })
}

/// The C# keyword type an IDL primitive becomes.
///
/// Exhaustive, so a primitive added to the model without a C# spelling
/// is a compile error and not a member that silently takes another's.
///
/// `long double` is **not** routed here: it takes the generated
/// `Float128`. The arm below exists so the match is total and answers
/// for a caller that wants the nearest keyword type; `type_at` matches
/// it before it reaches this function.
///
/// The two rows to read twice are `Int32` and `Int64`. IDL `long` is 32
/// bits and C# `long` is 64; a mapping that carried the spelling across
/// would compile and write four octets where the peer reads eight.
#[must_use]
pub const fn csharp_primitive(kind: PrimitiveKind) -> csharp::Predefined {
    match kind {
        PrimitiveKind::Boolean => csharp::Predefined::Bool,
        // A C# `char` is an unsigned 16-bit code unit, which holds an
        // IDL `char`'s 0..255 and a `wchar`'s BMP code point alike. The
        // wire width is the codec's and is not this type's.
        PrimitiveKind::Char | PrimitiveKind::WChar => csharp::Predefined::Char,
        PrimitiveKind::Int8 => csharp::Predefined::SByte,
        PrimitiveKind::Octet | PrimitiveKind::UInt8 => csharp::Predefined::Byte,
        PrimitiveKind::Int16 => csharp::Predefined::Short,
        PrimitiveKind::UInt16 => csharp::Predefined::UShort,
        // IDL `long` — 32 bits, so `int`.
        PrimitiveKind::Int32 => csharp::Predefined::Int,
        PrimitiveKind::UInt32 => csharp::Predefined::UInt,
        // IDL `long long` — 64 bits, so `long`.
        PrimitiveKind::Int64 => csharp::Predefined::Long,
        PrimitiveKind::UInt64 => csharp::Predefined::ULong,
        PrimitiveKind::Float32 => csharp::Predefined::Float,
        PrimitiveKind::Float64 | PrimitiveKind::Float128 => csharp::Predefined::Double,
    }
}

/// The unsigned C# type a value of `bits` bits is held in.
///
/// Table 33 of XTypes 1.3 §7.4.1.1.3.2, one to one: C# has an unsigned
/// keyword type at every width, so nothing has to be widened to stay a
/// number and nothing is held as a pattern. This is where the column
/// most visibly parts from Java, whose `unsigned_holder` can only answer
/// `int` or `long`.
#[must_use]
pub const fn unsigned_holder(bits: u16) -> csharp::Predefined {
    match bits {
        0..=8 => csharp::Predefined::Byte,
        9..=16 => csharp::Predefined::UShort,
        17..=32 => csharp::Predefined::UInt,
        _ => csharp::Predefined::ULong,
    }
}

/// The type an `@optional` member of `target` takes.
///
/// A value type becomes `T?`, which is `Nullable<T>` and a different
/// type from `T`. A reference type is returned unchanged: it is nullable
/// already, and writing `T?` on one outside a `#nullable enable` context
/// is CS8632 — a warning about an annotation the compiler will not act
/// on, which is worse than no annotation at all.
///
/// # Errors
/// As [`csharp_member_type`].
fn optional(
    ast: &Ast,
    target: Id<TypeNode>,
    type_: csharp::Type,
) -> Result<csharp::Type, MaterialiseError> {
    if is_value_type(ast, target, 0)? {
        return Ok(csharp::Type::Nullable(Box::new(type_)));
    }
    Ok(type_)
}

/// Whether an `@optional` member of `target` is declared `T?`.
///
/// The write half of the mapping this file's fourth decision makes: a
/// value type is wrapped in `Nullable<T>` and a reference type is not.
/// The codec has to know, because `int?` has no implicit conversion to
/// `int` and `string?` has no `.Value` — the two halves cannot be
/// spelled the same way, and a backend that guessed would produce a
/// file `csc` refuses in one direction and accepts wrongly in neither.
///
/// A model type the resolver will not answer for is not wrapped, which
/// is the same answer `optional` gives it.
#[must_use]
pub fn is_nullable_wrapped(ast: &Ast, target: Id<TypeNode>) -> bool {
    is_value_type(ast, target, 0).unwrap_or(false)
}

/// Whether the C# type a model type becomes is a value type.
///
/// Asked of the model and not of the produced [`csharp::Type`]: a
/// generated `enum` and a generated `class` are both a
/// [`csharp::Type::Named`], and only the model knows which is which.
///
/// # Errors
/// [`MaterialiseError::Unevaluated`] when the chain nests past
/// [`MAX_TYPE_DEPTH`].
///
/// zerodds-lint: recursion-depth 64 (alias chains; bounded by `MAX_TYPE_DEPTH`)
fn is_value_type(ast: &Ast, id: Id<TypeNode>, depth: usize) -> Result<bool, MaterialiseError> {
    if depth > MAX_TYPE_DEPTH {
        return Err(MaterialiseError::Unevaluated {
            construct: format!("type, whose chain nests past {MAX_TYPE_DEPTH}"),
            at: "an @optional member".to_string(),
        });
    }
    let Ok(node) = ast.type_(id) else {
        return Ok(false);
    };
    Ok(match &node.kind {
        // Every keyword numeric, and the generated `Float128` struct.
        TypeKind::Primitive(_) => true,
        // Generated `enum`s, which are value types like any other.
        TypeKind::Enumeration { .. } | TypeKind::Bitmask { .. } => true,
        TypeKind::Alias { aliased } => match aliased.resolved() {
            Some(target) => is_value_type(ast, target, depth + 1)?,
            None => false,
        },
        // `string`, arrays and every generated class. A `native` type is
        // here too: its facet may name a value type, but a facet that
        // did and a `?` this file added would be a claim about a
        // spelling the IDL only handed through.
        TypeKind::String { .. }
        | TypeKind::Fixed { .. }
        | TypeKind::Sequence { .. }
        | TypeKind::Array { .. }
        | TypeKind::Map { .. }
        | TypeKind::Structure { .. }
        | TypeKind::Union { .. }
        | TypeKind::Bitset { .. }
        | TypeKind::Valuetype { .. }
        | TypeKind::ValueBox { .. }
        | TypeKind::Native
        | TypeKind::Any
        | TypeKind::ValueBase
        | TypeKind::ObjectRef { .. }
        | TypeKind::AnnotationType { .. } => false,
    })
}

/// What a `native` declaration maps to.
///
/// The facet if a `#pragma` or an annotation gave one, and `object`
/// otherwise — never a guess at what the name might mean.
fn native_type(ast: &Ast, id: Id<TypeNode>) -> csharp::Type {
    let mapping = ast
        .any_of_type(id)
        .ok()
        .and_then(|any| ast.facets(any))
        .and_then(|facets| facets.corba.as_ref())
        .and_then(|corba| corba.native_mapping.clone());
    match mapping {
        Some(spelling) => {
            let segments: Vec<&str> = spelling.split("::").flat_map(|s| s.split('.')).collect();
            csharp::Type::named(Path::new(&segments))
        }
        None => csharp::Type::Predefined(csharp::Predefined::Object),
    }
}

/// A named type's path, rooted at the global namespace.
///
/// `global::a.b.Point` and not `a.b.Point`. C# resolves a type name
/// against the enclosing namespaces first, so inside `namespace
/// common.a` the segment `a` binds to `common.a` before the global `a` —
/// and only in the corpora that happen to nest that way. `global::`
/// resolves at the global namespace and nothing in scope can capture it.
///
/// # Errors
/// [`MaterialiseError::NoModule`] when the model will not answer for a
/// module the type nests in.
fn qualified(ast: &Ast, id: Id<TypeNode>) -> Result<Path, MaterialiseError> {
    let node = ast.type_(id).map_err(|_| MaterialiseError::Unevaluated {
        construct: "type".to_string(),
        at: "a member".to_string(),
    })?;
    let mut segments = Vec::new();
    for (_, name) in module_chain(ast, node.module)? {
        segments.push(name);
    }
    segments.push(csharp::escape_keyword(&node.name));
    Ok(Path::rooted(
        &segments.iter().map(String::as_str).collect::<Vec<&str>>(),
    ))
}

/// The map entry class a `map<K,V>` needs a name for.
///
/// # Errors
/// As [`type_tag`].
pub fn map_entry_name(ast: &Ast, id: Id<TypeNode>) -> Result<String, MaterialiseError> {
    let Ok(node) = ast.type_(id) else {
        return Err(MaterialiseError::Unevaluated {
            construct: "map".to_string(),
            at: ENTRY_PREFIX.to_string(),
        });
    };
    let TypeKind::Map { key, value, .. } = &node.kind else {
        return Err(MaterialiseError::Unevaluated {
            construct: "map".to_string(),
            at: named(&node.name, node.node_id),
        });
    };
    let (Some(key), Some(value)) = (key.resolved(), value.resolved()) else {
        return Err(MaterialiseError::Unevaluated {
            construct: "map key or value".to_string(),
            at: named(&node.name, node.node_id),
        });
    };
    Ok(format!(
        "{ENTRY_PREFIX}{}_{}",
        type_tag(ast, key, 0)?,
        type_tag(ast, value, 0)?
    ))
}

/// The bound a `string`, a `sequence` or a `map` carries, as the suffix
/// that keeps two of them apart.
///
/// The tag also names the routine a hoisted collection's codec is
/// emitted under, and one body is emitted per name. With the bound left
/// out, `sequence<long>` and `sequence<long, 4>` are one name and
/// whichever was declared first decides: measured on
/// `@final struct S { sequence<long> b; sequence<long, 4> a; };`, the
/// generated C# held no `zd_over` check at all, so `a` accepted a
/// stream of any length; with the members the other way round both got
/// the bounded body, so `b` refused — and truncated — a length the IDL
/// puts no limit on. `string<4>` beside `string` and `wstring<4>`
/// beside `wstring` collapse the same way, onto the trimming call.
/// Either way the reader answered with a value the IDL does not
/// describe and said nothing, which is why the bound is part of the
/// name. The same suffix the Zig column carries.
///
/// An `array` needs nothing here: its `dims` are already in the tag,
/// under the `Array` arm below.
fn bound_tag(ast: &Ast, bound: &zerodds_ast::Bound) -> String {
    match bound.expr() {
        None => String::new(),
        // `_b` and not a bare `_n`: `seq_long_4` is what the `Array`
        // arm answers for an array of four `long`, and the two are
        // different wire forms.
        Some(expr) => match ast.eval_integral(expr) {
            Ok(limit) => format!("_b{limit}"),
            // As the array's `_x`: a bound that does not evaluate is
            // still not the unbounded form, and the materialiser
            // reports the expression that would not evaluate.
            Err(_) => "_bx".to_string(),
        },
    }
}

/// The name component an entry class takes from one of its halves.
///
/// The IDL kind and not the C# spelling. C# distinguishes `octet` from
/// `unsigned short` where Java cannot, so this column has fewer
/// collisions to avoid than that one — but `char` and `wchar` are both a
/// C# `char` and are one and two octets on the wire, and an entry class
/// named after the C# type would give `map<char,long>` and
/// `map<wchar,long>` one class. A codec written against it would put one
/// of the two out at the other's width with nothing to say it had. An
/// alias is followed through, because a typedef is transparent here and
/// the two names would otherwise describe one class twice.
///
/// # Errors
/// [`MaterialiseError`] when a module or an array dimension will not
/// answer.
///
/// zerodds-lint: recursion-depth 64 (collection nesting; bounded by `MAX_TYPE_DEPTH`)
pub fn type_tag(ast: &Ast, id: Id<TypeNode>, depth: usize) -> Result<String, MaterialiseError> {
    if depth > MAX_TYPE_DEPTH {
        return Err(MaterialiseError::Unevaluated {
            construct: format!("type, whose name nests past {MAX_TYPE_DEPTH}"),
            at: ENTRY_PREFIX.to_string(),
        });
    }
    let Ok(node) = ast.type_(id) else {
        return Ok("unknown".to_string());
    };
    Ok(match &node.kind {
        TypeKind::Primitive(kind) => kind.as_str().replace(' ', "_"),
        TypeKind::String {
            width: CharWidth::Char,
            bound,
        } => format!("string{}", bound_tag(ast, bound)),
        TypeKind::String {
            width: CharWidth::WChar,
            bound,
        } => format!("wstring{}", bound_tag(ast, bound)),
        TypeKind::Fixed { digits, scale } => format!("fixed_{digits}_{scale}"),
        TypeKind::Sequence { element, bound } => match element.resolved() {
            Some(element) => format!(
                "seq{}_{}",
                bound_tag(ast, bound),
                type_tag(ast, element, depth + 1)?
            ),
            None => format!("seq_unknown{}", bound_tag(ast, bound)),
        },
        TypeKind::Array { element, dims } => {
            let mut name = match element.resolved() {
                Some(element) => format!("arr_{}", type_tag(ast, element, depth + 1)?),
                None => "arr_unknown".to_string(),
            };
            for dim in dims {
                match ast.eval_integral(*dim) {
                    Ok(length) => name.push_str(&format!("_{length}")),
                    // A dimension that does not evaluate gets no number
                    // rather than a zero: the name has to differ from a
                    // `[0]` array's, and `declare` reports the failure by
                    // name.
                    Err(_) => name.push_str("_x"),
                }
            }
            name
        }
        // The entry class's own name carries no bound — two maps that
        // differ only in bound hold the same pairs — but the tag does,
        // for the routine-name reason in [`bound_tag`].
        TypeKind::Map { key, value, bound } => {
            let key = match key.resolved() {
                Some(key) => type_tag(ast, key, depth + 1)?,
                None => "unknown".to_string(),
            };
            let value = match value.resolved() {
                Some(value) => type_tag(ast, value, depth + 1)?,
                None => "unknown".to_string(),
            };
            format!("map{}_{key}_{value}", bound_tag(ast, bound))
        }
        TypeKind::Alias { aliased } => match aliased.resolved() {
            Some(target) => type_tag(ast, target, depth + 1)?,
            None => "unknown".to_string(),
        },
        TypeKind::ValueBox { boxed } => match boxed.resolved() {
            Some(target) => format!("box_{}", type_tag(ast, target, depth + 1)?),
            None => "box_unknown".to_string(),
        },
        TypeKind::Structure { .. }
        | TypeKind::Union { .. }
        | TypeKind::Enumeration { .. }
        | TypeKind::Bitmask { .. }
        | TypeKind::Bitset { .. }
        | TypeKind::Valuetype { .. }
        | TypeKind::Native => {
            let mut segments = Vec::new();
            for (_, name) in module_chain(ast, node.module)? {
                segments.push(name);
            }
            segments.push(csharp::escape_keyword(&node.name));
            // A `@` is a lexical escape and not part of a name; leaving
            // it in would put one in the middle of a class name, where
            // it is not an escape at all.
            segments.join("_").replace('@', "")
        }
        TypeKind::Any => "any".to_string(),
        TypeKind::ValueBase => "valuebase".to_string(),
        TypeKind::ObjectRef { .. } => "objectref".to_string(),
        TypeKind::AnnotationType { .. } => "annotation".to_string(),
    })
}

// -- constants ------------------------------------------------------------

/// The C# expression a constant's value takes.
///
/// The declared type already carries the three string-shaped forms:
/// [`csharp_member_type`] answers `string` for a `string`, a `wstring`
/// and a `fixed`, so unlike C++ nothing here has to substitute a type.
/// What it does have to do is pick the right literal for each and refuse
/// the one C# has no literal for.
///
/// `long double` is refused by name. .NET has no 128-bit floating type,
/// the generated `Float128` is a struct with no literal and no `const`
/// form at all, and a `double` would put a different number in the file
/// than the IDL wrote.
///
/// # Errors
/// [`MaterialiseError`] naming the constant and what will not hold it.
fn constant_expr(
    type_: &csharp::Type,
    value: &zerodds_ast::ConstValue,
    at: &str,
) -> Result<csharp::Expr, MaterialiseError> {
    use zerodds_ast::ConstValue;
    let refuse = |construct: String| MaterialiseError::NoLanguageForm {
        construct,
        language: "C#",
        at: at.to_string(),
    };
    match value {
        ConstValue::Integer(number) => literal_of(type_, *number, at),
        ConstValue::Boolean(flag) => Ok(csharp::Expr::Literal(Literal::Bool(*flag))),
        ConstValue::Char(character) => Ok(csharp::Expr::Literal(Literal::Char(*character))),
        // A C# `char` is one UTF-16 code unit, so a code point above the
        // Basic Multilingual Plane does not fit one and is refused
        // rather than truncated into a lone surrogate.
        ConstValue::WChar(point) => match constant::wchar_char(*point)
            .filter(|character| u32::from(*character) <= u32::from(u16::MAX))
        {
            Some(character) => Ok(csharp::Expr::Literal(Literal::Char(character))),
            None => Err(refuse(format!(
                "the code point U+{point:04X}, which a C# `char` does not hold"
            ))),
        },
        ConstValue::Float { mantissa, exponent } => {
            let csharp::Type::Predefined(
                predefined @ (csharp::Predefined::Float | csharp::Predefined::Double),
            ) = type_
            else {
                return Err(refuse(String::from(
                    "a `long double` constant, which has no C# literal",
                )));
            };
            let mut text = constant::float_text(mantissa, exponent.as_deref());
            // Without the suffix a real literal is a `double`, and
            // `const float f = 1.5;` is a compile error rather than a
            // narrowing C# performs for you.
            if matches!(predefined, csharp::Predefined::Float) {
                text.push('f');
            }
            Ok(csharp::Expr::Literal(Literal::Float(text)))
        }
        ConstValue::Fixed { digits, scale } => Ok(csharp::Expr::Literal(Literal::Str(
            constant::fixed_decimal(digits, *scale),
        ))),
        ConstValue::String(text) => Ok(csharp::Expr::Literal(Literal::Str(text.clone()))),
        ConstValue::WString(points) => match constant::wstring_utf8(points) {
            Some(text) => Ok(csharp::Expr::Literal(Literal::Str(text))),
            None => Err(refuse(String::from(
                "a `wstring` constant holding a lone surrogate",
            ))),
        },
        // `ConstValue` is `#[non_exhaustive]`, so a form added to the
        // model reaches here. Refused by name rather than given a
        // plausible default: a constant emitted as the wrong literal is
        // a field that disagrees with the IDL and compiles.
        _ => Err(refuse(String::from(
            "a constant form this binding has no literal for",
        ))),
    }
}

/// The integer literal a constant of `type_` takes.
///
/// A named type here is a generated `enum`, and the value is cast to it:
/// an explicit enumeration conversion from a constant is itself a
/// constant expression, so it may initialise a `const`.
///
/// No suffix is written. A C# integer literal takes the first of `int`,
/// `uint`, `long` and `ulong` that holds it (ECMA-334 §6.4.5.3), and an
/// implicit constant conversion carries an in-range `int` constant to
/// every narrower type — so the decimal alone lands in the declared type
/// under every one of them. `long.MinValue` is the one special case, and
/// §6.4.5.3 spells that one out too.
///
/// # Errors
/// [`MaterialiseError::NoLanguageForm`] when the value does not fit the
/// type — never a truncation.
fn literal_of(
    type_: &csharp::Type,
    value: i128,
    at: &str,
) -> Result<csharp::Expr, MaterialiseError> {
    let (predefined, enum_path) = match type_ {
        csharp::Type::Predefined(predefined) => (*predefined, None),
        // A constant of enumerated type: the value is the enumerator's,
        // and `int` is what XTypes 1.3 7.4.3.5.3 makes it.
        csharp::Type::Named { path, .. } => (csharp::Predefined::Int, Some(path.clone())),
        _ => {
            return Err(MaterialiseError::Unevaluated {
                construct: "constant's value, which is not an integer".to_string(),
                at: at.to_string(),
            });
        }
    };
    let literal = match predefined {
        csharp::Predefined::Bool => {
            return Ok(csharp::Expr::Literal(Literal::Bool(value != 0)));
        }
        csharp::Predefined::Char => {
            let point = in_range(value, 0, i128::from(u16::MAX), "char", at)?;
            return Ok(csharp::Expr::Cast {
                type_: Box::new(csharp::Type::Predefined(csharp::Predefined::Char)),
                value: Box::new(csharp::Expr::Literal(Literal::int(point))),
            });
        }
        csharp::Predefined::SByte => in_range(value, -128, 127, "sbyte", at)?,
        csharp::Predefined::Byte => in_range(value, 0, 255, "byte", at)?,
        csharp::Predefined::Short => in_range(
            value,
            i128::from(i16::MIN),
            i128::from(i16::MAX),
            "short",
            at,
        )?,
        csharp::Predefined::UShort => in_range(value, 0, i128::from(u16::MAX), "ushort", at)?,
        csharp::Predefined::Int => {
            in_range(value, i128::from(i32::MIN), i128::from(i32::MAX), "int", at)?
        }
        csharp::Predefined::UInt => in_range(value, 0, i128::from(u32::MAX), "uint", at)?,
        csharp::Predefined::Long => in_range(
            value,
            i128::from(i64::MIN),
            i128::from(i64::MAX),
            "long",
            at,
        )?,
        csharp::Predefined::ULong => in_range(value, 0, i128::from(u64::MAX), "ulong", at)?,
        // An integral value declared as a real: the literal is the
        // number, and C# widens an int constant to it.
        csharp::Predefined::Float | csharp::Predefined::Double => value,
        _ => {
            return Err(MaterialiseError::Unevaluated {
                construct: "constant's value, which is not an integer".to_string(),
                at: at.to_string(),
            });
        }
    };
    let expr = csharp::Expr::Literal(Literal::int(literal));
    match enum_path {
        None => Ok(expr),
        Some(path) => Ok(csharp::Expr::Cast {
            type_: Box::new(csharp::Type::named(path)),
            value: Box::new(expr),
        }),
    }
}

/// `value`, or the failure that says it does not fit.
///
/// # Errors
/// [`MaterialiseError::NoLanguageForm`], naming the value and the type
/// that will not hold it. Truncating instead would put a number nobody
/// wrote into the output.
fn in_range(
    value: i128,
    low: i128,
    high: i128,
    type_: &str,
    at: &str,
) -> Result<i128, MaterialiseError> {
    if value < low || value > high {
        return Err(MaterialiseError::NoLanguageForm {
            construct: format!("the value {value}, which a C# `{type_}` does not hold"),
            language: "C#",
            at: at.to_string(),
        });
    }
    Ok(value)
}

// -- shared -------------------------------------------------------------

/// A model node's name, or its identity when the node is anonymous.
///
/// An array type declared inline has no name of its own, and an error
/// that said only "the array length does not evaluate" would leave the
/// reader to find which array.
fn named(name: &str, node: NodeId) -> String {
    if name.is_empty() {
        format!("{node:?}")
    } else {
        name.to_string()
    }
}

/// Whether the rules created a node of `type_name` for `node`.
///
/// The same two-hop walk the other materialisers do: a correspondence is
/// an ordinary node with a link to the anchor and a link to what was
/// created.
fn ghost_created_for(applied: &Applied, node: NodeId, type_name: &str) -> Option<GhostId> {
    let anchor = applied
        .origin
        .iter()
        .find(|(_, model)| **model == node)
        .map(|(ghost, _)| *ghost)?;
    let typ = applied.graph.types.lookup(type_name)?;
    applied
        .graph
        .nodes_of_type(typ)
        .map(|created| created.id)
        .find(|created| {
            applied.graph.connected(&anchor, created)
                || applied
                    .graph
                    .parts(&anchor)
                    .any(|part| applied.graph.connected(&part.other, created))
        })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;
    use crate::rule;
    use crate::{ModelGraph, apply};
    use std::path::{Path as FsPath, PathBuf};

    /// IDL in, one C# file out.
    fn render(idl: &str, path: &str) -> Result<String, String> {
        let parsed = zerodds_idl_parse::parse(idl);
        if parsed.has_errors() {
            return Err(format!("parse: {:?}", parsed.diagnostics()));
        }
        let lowered = zerodds_idl_lower::lower(&parsed.syntax(), path);
        if !lowered.diagnostics.is_empty() {
            return Err(format!("lower: {:?}", lowered.diagnostics));
        }
        let mut ast = lowered.ast;
        ast.resolve_all_by_name()
            .map_err(|error| format!("resolve: {error}"))?;
        let applied = apply(&ModelGraph::of(&ast), &rule::csharp::catalogue())
            .map_err(|error| format!("apply: {error}"))?;
        let mut file =
            csharp_file(&ast, &applied).map_err(|error| format!("materialise: {error}"))?;
        file.doc = Doc::lines(&[
            "Generated by zerodds-lang-rules. Do not edit.",
            "",
            "The C# shape of one IDL file. The wire rules are not here - they",
            "come from DDS-XTypes 1.3 and belong to the codec.",
        ]);
        Ok(zerodds_lang_render::csharp::file(&file))
    }

    /// The same, for a source with no file behind it.
    fn declarations(idl: &str) -> String {
        match render(idl, "test.idl") {
            Ok(text) => text,
            Err(why) => panic!("{why}"),
        }
    }

    /// Every corpus case, by name.
    fn corpus() -> Vec<(String, PathBuf)> {
        let root = FsPath::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/interop-corpus");
        let mut out = Vec::new();
        for entry in std::fs::read_dir(&root).expect("the corpus directory") {
            let entry = entry.expect("a corpus entry");
            let idl = entry.path().join("type.idl");
            if idl.is_file() {
                out.push((entry.file_name().to_string_lossy().into_owned(), idl));
            }
        }
        out.sort();
        out
    }

    /// The whole corpus, through every layer, with nothing skipped.
    ///
    /// A rendered file is not a compiled one. `ZD_CSHARP_OUT` names a
    /// directory to write the sources into, and a C# compiler over that
    /// directory is the second half of the check — a file that does not
    /// compile is a materialiser defect, not a corpus defect.
    #[test]
    fn every_corpus_case_renders_or_names_its_failure() {
        let cases = corpus();
        assert!(
            cases.len() > 300,
            "the corpus is not where it was: {} cases",
            cases.len()
        );
        let out = std::env::var("ZD_CSHARP_OUT").ok();
        let mut failures = Vec::new();
        for (name, path) in cases {
            let source = std::fs::read_to_string(&path).expect("a corpus source");
            match render(&source, &path.display().to_string()) {
                Ok(text) => {
                    if let Some(dir) = &out {
                        let case = FsPath::new(dir).join(&name);
                        std::fs::create_dir_all(&case).expect("the output directory");
                        std::fs::write(case.join("ZdTypes.cs"), text).expect("the output file");
                    }
                }
                Err(why) => failures.push(format!("{name}: {why}")),
            }
        }

        // The five constructs no corpus case declares. The corpus is a
        // wire corpus, so it carries what goes on the wire; `const`,
        // `exception`, `valuetype`, `native` and `interface` are in the
        // catalogue all the same, and a column whose compile gate ran
        // only over the corpus would be claiming they compile rather
        // than showing it. Dumped beside the cases, so whatever compiler
        // walks that directory walks these too.
        match render(UNREACHABLE_BY_THE_CORPUS, "unreachable-constructs.idl") {
            Ok(text) => {
                if let Some(dir) = &out {
                    let case = FsPath::new(dir).join("_unreachable-constructs");
                    std::fs::create_dir_all(&case).expect("the output directory");
                    std::fs::write(case.join("ZdTypes.cs"), text).expect("the output file");
                }
            }
            Err(why) => failures.push(format!("_unreachable-constructs: {why}")),
        }

        let failures = crate::adapter::without_the_front_end_refusals(failures);
        assert!(failures.is_empty(), "{failures:#?}");
    }

    /// One source carrying every construct the wire corpus does not.
    const UNREACHABLE_BY_THE_CORPUS: &str = "
        module reach {
          const long MAX = 10;
          const string NAME = \"zerodds\";
          const double RATIO = 1.5;
          typedef sequence<long> Row;
          native Handle;
          exception Overrun { long limit; string message; };
          valuetype Node { public long id; public string label; };
          interface Robot {
            long move(in long distance, out double travelled, inout long budget);
            void halt();
          };
          @final struct Sample { Row r; Handle h; Node n; long tail; };
        };
    ";

    #[test]
    fn every_idl_primitive_reaches_the_csharp_type_that_holds_it() {
        let generated = declarations(
            "@final struct P {
               boolean a; octet b; char c; wchar d;
               int8 e; uint8 f; short g; unsigned short h;
               long i; unsigned long j; long long k; unsigned long long l;
               float m; double n; long double o;
             };",
        );
        for expected in [
            "public bool A;",
            "public byte B;",
            "public char C;",
            "public char D;",
            "public sbyte E;",
            "public byte F;",
            "public short G;",
            "public ushort H;",
            // IDL `long` is 32 bits; a C# `long` is 64.
            "public int I;",
            "public uint J;",
            "public long K;",
            "public ulong L;",
            "public float M;",
            "public double N;",
            "public global::ZeroDds.Idl.Float128 O;",
        ] {
            assert!(generated.contains(expected), "{expected}\n{generated}");
        }
    }

    #[test]
    fn every_field_carries_the_idl_name_it_was_folded_from() {
        // The fold is what makes the attribute load-bearing: a member's
        // ID under @autoid(HASH) is the MD5 of `venue`, not of `Venue`.
        let generated = declarations("@final struct P { long venue; long my_field; };");
        assert!(
            generated.contains("[global::ZeroDds.Idl.IdlName(\"venue\")]"),
            "{generated}"
        );
        assert!(generated.contains("public int Venue;"), "{generated}");
        // Only the first letter moves: a full PascalCase fold is not
        // injective over the names IDL allows in one scope.
        assert!(generated.contains("public int My_field;"), "{generated}");
    }

    #[test]
    fn two_members_that_differ_only_after_the_fold_stay_two_members() {
        // `my_field` and `myField` are both legal in one IDL struct and
        // both fold to `MyField` under a full PascalCase. Under this
        // one they do not collide at all.
        let generated = declarations("@final struct P { long my_field; long myField; };");
        assert!(generated.contains("public int My_field;"), "{generated}");
        assert!(generated.contains("public int MyField;"), "{generated}");
    }

    #[test]
    fn a_member_that_would_collide_with_its_type_cannot_come_from_idl() {
        // CS0542 forbids a member sharing the simple name of its type,
        // and `fields_of` moves such a member to `Name_` with an
        // `[IdlName]` attribute keeping the IDL spelling.
        //
        // That move is no longer reachable **from IDL**, and the test
        // says so rather than asserting a shape nothing can produce.
        // `capitalise` lifts the first letter and nothing else, so a
        // member folds onto its type's name only when the two differ in
        // that letter alone — which IDL 4.2 §7.2.3 refuses, and the model
        // refuses it before any backend runs. The same argument covers a
        // collision between two members.
        //
        // The logic stays because a model does not have to come from
        // IDL: `dynamic::to_ast` builds one from a peer's TypeObjects,
        // where §7.2.3 was that peer's rule to keep and not ours to
        // assume.
        let refused = render("@final struct Point { long point; };", "test.idl");
        assert!(
            refused.is_err(),
            "§7.2.3 stops this before C# sees it: {refused:?}"
        );

        // The neighbouring shape that IDL does admit, to show where the
        // line runs: an underscore is not a case difference, and both
        // names survive with distinct C# spellings.
        let generated = declarations("@final struct MyField { long my_field; };");
        assert!(
            generated.contains("public sealed class MyField"),
            "{generated}"
        );
        assert!(generated.contains("public int My_field;"), "{generated}");
    }

    #[test]
    fn a_sequence_and_an_array_are_both_csharp_arrays() {
        let generated = declarations(
            "@final struct P { sequence<long> s; long a[3]; long g[2][3]; sequence<string> t; };",
        );
        assert!(generated.contains("public int[] S;"), "{generated}");
        assert!(generated.contains("public int[] A;"), "{generated}");
        assert!(generated.contains("public int[][] G;"), "{generated}");
        assert!(generated.contains("public string[] T;"), "{generated}");
    }

    #[test]
    fn a_map_is_an_array_of_a_generated_entry_class() {
        let generated = declarations("@final struct P { map<long, string> m; };");
        // The tag is the IDL kind's spelling: `char` and `wchar` are
        // both a C# `char` and are one and two octets on the wire.
        assert!(
            generated.contains("class ZdEntry_long_string"),
            "{generated}"
        );
        assert!(generated.contains("public int Key;"), "{generated}");
        assert!(generated.contains("public string Value;"), "{generated}");
        assert!(
            generated.contains("public global::ZdEntry_long_string[] M;"),
            "{generated}"
        );
        // No field is a Dictionary; the entry order is the sample's.
        // (The entry class's own prose names the type it is not, so the
        // check is on a field and not on the text.)
        assert!(
            !generated.contains("public global::System.Collections"),
            "{generated}"
        );
    }

    #[test]
    fn two_maps_of_one_shape_are_one_entry_class() {
        let generated = declarations("@final struct P { map<long, long> a; map<long, long> b; };");
        assert_eq!(generated.matches("class ZdEntry_long_long").count(), 1);
    }

    #[test]
    fn two_maps_whose_csharp_spelling_coincides_are_still_two_entry_classes() {
        // `char` and `wchar` are both a C# `char` and are one and two
        // octets on the wire. One entry class for both would put one of
        // them out at the other's width.
        let generated = declarations("@final struct P { map<char, long> a; map<wchar, long> b; };");
        assert!(generated.contains("class ZdEntry_char_long"), "{generated}");
        assert!(
            generated.contains("class ZdEntry_wchar_long"),
            "{generated}"
        );
    }

    #[test]
    fn an_optional_value_type_is_nullable_and_a_reference_one_is_not() {
        let generated = declarations(
            "@mutable struct P {
               @optional long a; @optional string b; @optional long long c;
             };",
        );
        // An IDL `long` is 32 bits, so the nullable is `int?`.
        assert!(generated.contains("public int? A;"), "{generated}");
        // A `string` is a reference and nullable already; `string?`
        // outside a #nullable context is CS8632.
        assert!(generated.contains("public string B;"), "{generated}");
        assert!(!generated.contains("string?"), "{generated}");
        assert!(generated.contains("public long? C;"), "{generated}");
    }

    #[test]
    fn an_optional_enum_is_nullable_because_an_enum_is_a_value_type() {
        let generated = declarations(
            "enum Colour { RED, GREEN };
             @mutable struct P { @optional Colour c; };",
        );
        assert!(
            generated.contains("public global::Colour? C;"),
            "{generated}"
        );
    }

    #[test]
    fn a_union_is_a_class_with_a_discriminator_and_one_field_per_case() {
        let generated = declarations(
            "union Choice switch (long) {
               case 1: long whole;
               case 2: long other;
               default: double fraction;
             };
             @final struct P { Choice c; };",
        );
        assert!(generated.contains("public int _d;"), "{generated}");
        assert!(generated.contains("public int Whole;"), "{generated}");
        assert!(generated.contains("public int Other;"), "{generated}");
        assert!(generated.contains("public double Fraction;"), "{generated}");
        assert!(
            generated.contains("public global::Choice C;"),
            "{generated}"
        );
    }

    #[test]
    fn an_enumeration_is_a_real_enum_and_a_member_of_it_is_that_type() {
        let generated = declarations(
            "enum Colour { RED, GREEN, BLUE };
             @final struct P { Colour c; };",
        );
        assert!(
            generated.contains("public enum Colour : int"),
            "{generated}"
        );
        assert!(generated.contains("RED = 0,"), "{generated}");
        assert!(generated.contains("BLUE = 2,"), "{generated}");
        // The type and not an `int`: a C# enum member's identity is the
        // integer it is given, which is the Java column's objection.
        assert!(
            generated.contains("public global::Colour C;"),
            "{generated}"
        );
    }

    #[test]
    fn an_explicit_enumerator_value_is_written_out() {
        let generated = declarations("enum E { @value(-1) LOW, @value(70000) HIGH };");
        assert!(generated.contains("LOW = -1,"), "{generated}");
        assert!(generated.contains("HIGH = 70000,"), "{generated}");
    }

    #[test]
    fn a_bitmask_is_a_flags_enum_over_the_holder_its_bound_calls_for() {
        let narrow =
            declarations("@bit_bound(8) bitmask Small { @position(0) A, @position(7) B };");
        assert!(narrow.contains("public enum Small : byte"), "{narrow}");
        assert!(narrow.contains("[global::System.Flags]"), "{narrow}");
        assert!(narrow.contains("B = 128,"), "{narrow}");
        let wide = declarations("@bit_bound(64) bitmask Big { @position(0) A, @position(40) B };");
        assert!(wide.contains("public enum Big : ulong"), "{wide}");
        // The value and not `1 << 40`: a C# shift is typed by its left
        // operand, and `1 << 40` is an int shift evaluating to 256.
        assert!(wide.contains("B = 1099511627776,"), "{wide}");
    }

    #[test]
    fn a_bitset_is_a_class_of_one_field_per_bitfield() {
        let generated = declarations(
            "bitset Packed { bitfield<3> low; bitfield<5> high; };
             @final struct P { Packed value; };",
        );
        assert!(generated.contains("public byte Low;"), "{generated}");
        assert!(generated.contains("public byte High;"), "{generated}");
        assert!(
            generated.contains("public global::Packed Value;"),
            "{generated}"
        );
    }

    #[test]
    fn a_fixed_is_a_string_and_not_a_decimal() {
        let generated = declarations(
            "typedef fixed<9,3> Money;
             @final struct P { Money amount; };",
        );
        assert!(generated.contains("public string Amount;"), "{generated}");
        // No field takes one. (`decimal` appears in the support
        // namespace's prose, which names the type it is not.)
        assert!(!generated.contains("public decimal"), "{generated}");
    }

    #[test]
    fn a_typedef_is_a_using_alias_and_every_use_takes_the_resolved_type() {
        let generated = declarations(
            "typedef sequence<long> Row;
             @final struct P { Row r; };",
        );
        assert!(generated.contains("using Row = int[];"), "{generated}");
        // The use takes the resolved type: a `using` alias may not be
        // built on another one in the same scope.
        assert!(generated.contains("public int[] R;"), "{generated}");
    }

    #[test]
    fn a_module_is_a_namespace_and_two_modules_do_not_collide() {
        let generated = declarations(
            "module a { @final struct P { long x; }; };
             module b { @final struct P { double y; }; };
             @final struct Q { a::P one; b::P two; };",
        );
        assert!(generated.contains("namespace a"), "{generated}");
        assert!(generated.contains("namespace b"), "{generated}");
        assert!(generated.contains("public global::a.P One;"), "{generated}");
        assert!(generated.contains("public global::b.P Two;"), "{generated}");
    }

    #[test]
    fn a_module_reopened_beside_a_nested_one_is_one_namespace_block() {
        let generated = declarations(
            "module common {
               module util { @final struct Location { double x; }; };
               @final struct Robot { ::common::util::Location where_; };
             };",
        );
        assert_eq!(
            generated.matches("namespace common\n").count(),
            1,
            "{generated}"
        );
        assert!(
            generated.contains("public global::common.util.Location Where_;"),
            "{generated}"
        );
    }

    #[test]
    fn an_identifier_that_is_a_csharp_keyword_is_escaped_with_an_at_sign() {
        let generated = declarations(
            "module namespace {
               enum class { template, virtual, friend };
               @final struct type { long new; };
             };",
        );
        assert!(generated.contains("namespace @namespace"), "{generated}");
        assert!(generated.contains("enum @class"), "{generated}");
        assert!(generated.contains("class type"), "{generated}");
        // A member is folded before it is escaped, and `New` is not a
        // keyword — so the escape never fires on a member that the fold
        // has already moved out of the keyword space. The IDL spelling
        // is on the attribute either way.
        assert!(generated.contains("public int New;"), "{generated}");
        assert!(
            generated.contains("[global::ZeroDds.Idl.IdlName(\"new\")]"),
            "{generated}"
        );
        // `virtual` is a C# keyword and moves; `template` and `friend`
        // are C++ keywords and are identifiers here. Enumerator names
        // are not folded, so the escape is what they meet.
        assert!(generated.contains("@virtual = 1,"), "{generated}");
        assert!(generated.contains("template = 0,"), "{generated}");
    }

    #[test]
    fn a_constant_becomes_a_const_field_of_a_holder_class() {
        let generated = declarations(
            "const long MAX = 10;
             const string NAME = \"zerodds\";
             const unsigned long WIDE = 4294967295;",
        );
        assert!(generated.contains("class ZdConstants"), "{generated}");
        assert!(
            generated.contains("public const int MAX = 10;"),
            "{generated}"
        );
        assert!(
            generated.contains("public const string NAME = \"zerodds\";"),
            "{generated}"
        );
        // A C# integer literal takes the first of int, uint, long, ulong
        // that holds it, so no suffix is needed.
        assert!(
            generated.contains("public const uint WIDE = 4294967295;"),
            "{generated}"
        );
    }

    #[test]
    fn a_construct_with_no_csharp_data_form_is_named_and_not_left_out() {
        // The member is not dropped and no `object` is substituted for
        // it: a class missing a field that reports success is the defect
        // this materialiser is arranged against.
        for (idl, construct) in [
            ("@final struct P { any a; long tail; };", "any"),
            ("@final struct P { Object o; long tail; };", "Object"),
        ] {
            let failure = render(idl, "test.idl").expect_err(construct);
            assert!(failure.contains(construct), "{failure}");
            assert!(failure.contains("C#"), "{failure}");
        }
    }

    #[test]
    fn a_name_that_would_capture_the_rooted_references_is_refused() {
        for module in ["System", "ZeroDds"] {
            let idl = format!("module {module} {{ @final struct P {{ long x; }}; }};");
            let failure = render(&idl, "test.idl").expect_err(module);
            assert!(failure.contains(module), "{failure}");
        }
    }

    #[test]
    fn an_unevaluated_array_length_is_named_and_not_substituted() {
        // The C and Java columns fail on the same input; the three
        // columns' failure sets stay comparable.
        let failure = render("@final struct P { long a[UNKNOWN]; };", "test.idl")
            .expect_err("an array whose length does not evaluate is refused");
        assert!(!failure.is_empty(), "{failure}");
    }

    #[test]
    fn an_interface_is_an_interface_of_signatures_with_the_idl_passing_modes() {
        // No corpus case declares one, so this is what says the
        // `interface-to-interface` rule materialises rather than
        // claiming a coverage cell that produces nothing.
        let generated = declarations(
            "interface Robot {
               long move(in long distance, out double travelled, inout long budget);
               void halt();
             };",
        );
        assert!(generated.contains("public interface Robot"), "{generated}");
        assert!(
            generated.contains("int Move(int distance, out double travelled, ref int budget);"),
            "{generated}"
        );
        assert!(generated.contains("void Halt();"), "{generated}");
        assert!(
            generated.contains("[global::ZeroDds.Idl.IdlName(\"move\")]"),
            "{generated}"
        );
    }

    #[test]
    fn an_exception_is_a_data_class_and_not_a_system_exception() {
        // No corpus case declares one either. An IDL member named
        // `message` folds onto `System.Exception.Message`, and hiding an
        // inherited member (CS0108) is not a decision the wire makes.
        let generated = declarations("exception Overrun { long limit; string message; };");
        assert!(
            generated.contains("public sealed class Overrun"),
            "{generated}"
        );
        assert!(
            !generated.contains(": global::System.Exception"),
            "{generated}"
        );
        assert!(generated.contains("public int Limit;"), "{generated}");
        assert!(generated.contains("public string Message;"), "{generated}");
    }

    #[test]
    fn a_valuetype_is_a_class_of_its_state_members() {
        let generated = declarations(
            "valuetype Node { public long id; public string label; };
             @final struct P { Node n; };",
        );
        assert!(
            generated.contains("public sealed class Node"),
            "{generated}"
        );
        assert!(generated.contains("public int Id;"), "{generated}");
        assert!(generated.contains("public string Label;"), "{generated}");
        assert!(generated.contains("public global::Node N;"), "{generated}");
    }

    #[test]
    fn a_native_type_takes_an_alias_and_object_where_no_facet_names_one() {
        let generated = declarations(
            "native Handle;
             @final struct P { Handle h; };",
        );
        assert!(generated.contains("using Handle = object;"), "{generated}");
        // The use takes the resolved type, as a typedef's does.
        assert!(generated.contains("public object H;"), "{generated}");
    }

    #[test]
    fn every_generated_file_carries_the_support_namespace() {
        let generated = declarations("@final struct P { long x; };");
        assert!(generated.contains("namespace ZeroDds.Idl"), "{generated}");
        assert!(
            generated.contains("class IdlNameAttribute : global::System.Attribute"),
            "{generated}"
        );
        assert!(
            generated.contains("public readonly struct Float128"),
            "{generated}"
        );
    }
}
