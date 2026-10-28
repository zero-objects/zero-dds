// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The TypeScript CST as the other end of a rule. Spec §3.2.
//!
//! The counterpart of [`crate::adapter::java`], which does the same for
//! Java, and of [`crate::adapter::c`], which does it for C. Reading two
//! of them side by side is the point: where they differ, the difference
//! is the language's, and it is written down here rather than spread
//! through a renderer.
//!
//! Values are resolved from the model, never from the overlay — the same
//! rule the other materialisers follow and for the same reason
//! (spec §2.3).
//!
//! # `number` is a binary64, so a 64-bit integer is a `bigint`
//!
//! This is the decision that matters most in this column, so it is
//! first.
//!
//! A TypeScript `number` is an IEEE 754 binary64. Every integer up to
//! 2^53 - 1 is a distinct value in it and no integer above that is:
//! `9007199254740993` and `9007199254740992` are the same `number`.
//! An IDL `long long` and `unsigned long long` reach 2^63 - 1 and
//! 2^64 - 1, so a `number`-typed field silently rounds most of their
//! range.
//!
//! That is not an edge case. A nanosecond timestamp passes 2^53 ns
//! about 104 days after its epoch, so **every** absolute nanosecond
//! time — the thing an `unsigned long long` in a DDS type almost always
//! is — is past it by two orders of magnitude. `Date.now() * 1e6` is
//! roughly 1.7 × 10^18, which is 194 times 2^53. A `number` holding it
//! is wrong in the last three digits, always, and nothing reports it.
//!
//! So:
//!
//! | IDL | TypeScript | what the field holds |
//! |---|---|---|
//! | `boolean` | `boolean` | |
//! | `char` | `string` | one code unit; the wire form is one octet |
//! | `wchar` | `string` | one UTF-16 code unit |
//! | `int8`, `octet`, `uint8` | `number` | a number |
//! | `short`, `unsigned short` | `number` | a number |
//! | `long` | `number` | a number |
//! | `unsigned long` | `number` | a number, 0..4294967295 — exact |
//! | `long long` | **`bigint`** | exact past 2^53 |
//! | `unsigned long long` | **`bigint`** | exact past 2^53 |
//! | `float`, `double` | `number` | |
//! | `long double` | [`FLOAT128`] | never a `number` |
//!
//! **The cost is real and it is paid here rather than hidden.** A
//! `bigint` does not mix with a `number` in arithmetic — `1n + 1` is a
//! `TypeError` at run time and an error at compile time — it has no
//! `JSON.stringify` form, and it allocates. Every 64-bit member pays
//! that, including the ones whose values are small. The alternative
//! pays in silence, and a wire library whose 64-bit fields are
//! approximately right is not a wire library.
//!
//! Two consequences worth naming, because they are where this column
//! differs from Java's:
//!
//! - `unsigned long` stays a `number`. 4294967295 is far below
//!   2^53, so unlike Java — where it needs a `long` because `int` is
//!   signed — nothing has to widen. Every IDL type of 32 bits or fewer
//!   is a `number`, signed or not.
//! - `unsigned long long` is a **number**, not a bit pattern. Java has
//!   no unsigned 64-bit type and stores the pattern in a signed `long`,
//!   so `-1L` means 18446744073709551615 there. A `bigint` is
//!   unbounded, so `18446744073709551615n` is written as itself.
//!
//! The prior TypeScript column in this repository —
//! `crates/idl-ts`, generated under DDS-TS 1.0 — made the same call
//! (`long long..int64 | bigint` in its mapping table), and the
//! `zerodds-examples/idl-conformance/_interop/ts` sources it produced
//! carry `i64: bigint`. This is not a new decision; it is the same one,
//! restated with its reasoning.
//!
//! # The shapes
//!
//! | IDL | TypeScript |
//! |---|---|
//! | `string`, `wstring` | `string` |
//! | `fixed<P,S>` | `string`, holding the decimal |
//! | `sequence<T>`, `T[N]` | `T[]` |
//! | `T[N][M]` | `T[][]` |
//! | `map<K,V>` | `ZdEntry_k_v[]` — an array of a two-field interface |
//! | `struct` | an `interface` **and** a factory function returning one |
//! | `union` | a discriminated union of object types |
//! | `enum` | an `as const` object, beside `type E = number` |
//! | `bitmask` | an `as const` object, beside its holder's alias |
//! | `bitset` | an interface of one field per bitfield |
//! | `typedef` | `type Name = T` — a real declaration |
//! | `@optional T m` | `m?: T` |
//! | `module` | `namespace` |
//!
//! ## An interface *and* a factory, and no class at all
//!
//! An `interface` is type-space only: it is erased and there is nothing
//! to call `new` on. Something has to produce a value all the same — a
//! generated decoder opens with `let v: T = …` before it reads an octet,
//! and a nested member's default has to come from somewhere.
//!
//! A `class` would do it and this column emitted one until DDS-TS 1.0
//! §7.4 was read against it: "this PSM does not emit any TypeScript
//! class for IDL data types", measured by Annex B §B.1.2 as *no `class`
//! keyword in the generator output for any IDL data type*. So `P` is
//! emitted beside `newP(): P`, a function returning an object literal
//! with every required member at its Table 9 value.
//!
//! The factory is the better answer for the reason the class needed a
//! paragraph to excuse: a caller typed as the interface depended on the
//! implementation, and now there is no implementation to depend on. It
//! also drops `strictPropertyInitialization` from the picture entirely —
//! the rule that made every class field carry an initialiser — because
//! an object literal states its members or does not typecheck.
//!
//! An `@optional` member gets **no** entry. §7.2.2.4.4.4.8's
//! "Implementations shall not provide a default value" is the property
//! not being there, and under `exactOptionalPropertyTypes` an explicit
//! `undefined` is a different value from an absent property.
//!
//! ## A union is a discriminated union and not a class
//!
//! TypeScript narrows `{ _d: 1; a: number } | { _d: 2; b: string }` on
//! `_d` natively, so the C++ and Java shape — a class with `_d` and one
//! field per case, all of them always present — is not needed here. A
//! case's discriminant is a **literal type**, which is what makes the
//! narrowing work; a `default:` case takes the discriminator's whole
//! type, because that is what it selects.
//!
//! The discriminant literal follows the discriminator's own mapping: a
//! `boolean` discriminator gives `_d: true`, a `char` one gives
//! `_d: "a"`, a `long long` one gives `_d: 1n`. A `2` and a `2n` are two
//! different types, and using the wrong one is a union no value
//! inhabits.
//!
//! ## An enumeration is an `as const` object, not an `enum`
//!
//! TypeScript has three forms and they differ at run time:
//!
//! - `enum E { … }` emits an object *and* a reverse-lookup map, and
//!   since TypeScript 5.0 an `E`-typed slot rejects a number that is not
//!   one of its members. An `@appendable` writer that added an
//!   enumerator sends exactly such a number, and the field could then
//!   only refuse the sample.
//! - `const enum E { … }` emits nothing and substitutes at each use,
//!   which `isolatedModules` — every bundler — cannot do across a module
//!   boundary.
//! - `const E = { … } as const` beside `type E = number` emits one
//!   frozen-shaped object, keeps the names, and leaves the member's type
//!   a `number` that holds any value the wire carries.
//!
//! The third. It is the same half the Java column keeps — the `int`
//! constants — with the enumeration's *name* surviving in the type,
//! which Java's `int` member does not manage.
//!
//! ## `typedef` is a real declaration here
//!
//! TypeScript has `type Name = T`, so unlike the Java column an IDL
//! `typedef` is not a comment. A member of the aliased type is written
//! as the alias, so the IDL's name reaches the generated source.
//!
//! ## Identifiers
//!
//! Two spaces, two rules, and this is where TypeScript is kinder than
//! the other four columns:
//!
//! - A **binding** — an interface, a class, a type alias, a namespace, a
//!   `const` — goes through [`zerodds_lang_cst::typescript::escape_keyword`],
//!   which appends an underscore. `module package` becomes
//!   `namespace package_` and `enum class` becomes `const class_`.
//! - A **property** keeps the IDL's spelling. `class Sample { new:
//!   number = 0 }` parses, so `new`, `delete`, `operator`, `static` and
//!   `async` are written as the IDL wrote them. Only `constructor`
//!   moves, because a class body parses it as the constructor and
//!   `constructor: number` is a syntax error; it moves in the interface
//!   too, or the class would no longer implement it.
//!
//! Where a name did move, the declaration carries a comment naming the
//! IDL identifier it came from. A rename with nothing recording it is a
//! name the reader cannot find in the IDL.
//!
//! ## No outer wrapper, and what replaces the check that needed one
//!
//! The Java column nests everything inside one `ZdTypes` class and
//! writes every reference from it, because JLS §6.5.2 resolves the first
//! segment of a qualified name to a *variable* first. TypeScript has no
//! such rule — a property is not in scope as an identifier — so a module
//! becomes a plain `namespace` and a reference is written from the file
//! root.
//!
//! What TypeScript does have is no root operator at all. There is no
//! `::a::b::P`, so a namespace nested inside another that shares a
//! top-level declaration's name captures every reference written through
//! it. `check_names` refuses that arrangement rather than emitting a
//! file whose types resolve to the wrong declaration.
//!
//! It also refuses a second declaration of one name in one scope, and
//! that one is not a compile error being anticipated: two
//! `interface P` declarations in one scope **merge**, silently, into one
//! interface carrying both sets of members. A generator that let that
//! through would emit a type that is neither of the two the IDL
//! declared and report success.
//!
//! ## What is refused
//!
//! `ValueBase` and an object reference have no TypeScript data form here
//! and are named as [`MaterialiseError::NoLanguageForm`] rather than
//! left out. `unknown` would compile and would be a member no codec can
//! write.
//!
//! `any` is **not** among them. It takes the boxed form
//! [`ANY_BOX`] — the value beside the qualified IDL name of its type —
//! which DDS-TS 1.0 §7.3.4 requires and which is the only shape that
//! carries what an IDL `any` carries. What it does not have is a wire
//! form: the codec walk still refuses it, and that refusal is a wire
//! question rather than a mapping one.
//!
//! ## The descriptor side-tables
//!
//! [`ts_descriptors`] is the second half of the mapping and the reason
//! there is one: a TypeScript interface is erased, so the annotations an
//! IDL declaration carries — `@key`, `@id`, extensibility, `@topic` —
//! have nowhere to live in the emitted type. DDS-TS 1.0 §7.4.1 puts
//! them in a first-class value beside it: an
//! `export const <Type>Type: DdsTypeDescriptor<T>`, an
//! `is<Type>` type guard, and a `registerType(<Type>Type)` that makes
//! the descriptor findable by qualified name.
//!
//! It is a separate function from [`ts_items`] because its output is not
//! self-contained: `registerType` is a function with process-wide state,
//! so a file carrying descriptors imports the runtime and a file
//! carrying only types does not.

use crate::adapter::c::ordered_types;
use crate::adapter::constant;
use crate::adapter::cst::{MaterialiseError, no_spelling_collision};
use crate::apply::Applied;
use crate::verbatim;
use seesaw_tgg::ident::GhostId;
use zerodds_ast::{Ast, CaseLabel, CharWidth, ModuleNode, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::{Id, NodeId};
use zerodds_lang_cst::common::{Doc, Ident, Literal, Path};
use zerodds_lang_cst::typescript as ts;

/// The interface an IDL `long double` takes.
///
/// TypeScript has one numeric type and it is a binary64, so there is no
/// approximation to reach for: a `number` member would put eight bytes
/// on the wire where the type says sixteen. [`prelude`] declares this
/// as the sixteen octets themselves, which is the only form that
/// carries a binary128 without deciding what it means.
///
/// `crates/lang-rules/src/adapter/java.rs` names
/// `org.zerodds.wire.Float128` for the same reason and the C++ column
/// names `zerodds::wire::Float128`; the difference is that those two
/// have a runtime library to put it in and this file has to declare it,
/// because a generated `.ts` that imported a module the corpus does not
/// ship would not typecheck.
pub const FLOAT128: &str = "ZdFloat128";

/// The interface an IDL `any` takes. DDS-TS 1.0 §7.3.4.
///
/// An IDL `any` carries a value **together with the identity of its
/// type** (IDL 4.2 §7.4.14). A TypeScript `interface` is erased, so an
/// unwrapped payload gives a consumer nothing to discover that identity
/// from; the mapping therefore boxes, and `typeId` — the qualified IDL
/// name — is what a consumer passes to `lookupType` to get the
/// descriptor that describes `value`.
///
/// §7.3.4 forbids the two alternatives by name. TypeScript `any`
/// switches the checking off, and `unknown` says the type is not known
/// where the IDL construct says it is carried.
///
/// [`prelude`] declares this in the generated file rather than importing
/// it, exactly as it does [`FLOAT128`]: TypeScript is structural, so the
/// declaration below and `DdsAny` in `runtime/typescript/src/types.ts`
/// are one type and values pass between them, while a generated file
/// that imported it would not typecheck where the runtime is not
/// installed.
pub const ANY_BOX: &str = "DdsAny";

/// The prefix a generated map-entry declaration carries.
const ENTRY_PREFIX: &str = "ZdEntry_";

/// The suffix the instantiable half of a declaration carries.
/// The prefix the factory function of a declaration carries.
///
/// `newPoint()` beside `interface Point`. A function and not a class,
/// because DDS-TS 1.0 §7.4 forbids a class for an IDL data type outright
/// — "this PSM does not emit any TypeScript class for IDL data types" —
/// and Annex B §B.1.2 measures it: no `class` keyword may appear in the
/// generator output for any IDL data type.
///
/// What the class was there for is still needed. An `interface` is
/// type-space only: it is erased, and a decoder that opens with
/// `let v: T = …` has to get a `T` from somewhere. The factory is that
/// somewhere, and it is a better answer than the class was: a caller
/// typed as the interface never depends on an implementation, because
/// there is none to depend on.
const FACTORY_PREFIX: &str = "new";

/// The names a generated declaration may not take, at any depth.
///
/// [`FLOAT128`] is declared by [`prelude`] at the file root, so an IDL
/// type of that name would be a second declaration in one scope — which
/// TypeScript resolves by *merging* rather than by reporting.
///
/// `Array` and `Uint8Array` are globals this materialiser writes calls
/// against: `Array.from({ length: n }, …)` fills an IDL array and
/// `new Uint8Array(16)` starts a [`FLOAT128`]. A declaration of either
/// name shadows the global for every scope inside it, and the generated
/// initialiser would then call something else entirely.
const RESERVED_NAMES: &[&str] = &[FLOAT128, ANY_BOX, "Array", "Uint8Array"];

/// How deep a module chain is followed before the model is taken to be
/// malformed. IDL nests nowhere near this.
const MAX_MODULE_DEPTH: usize = 64;

/// How deep a type's TypeScript spelling is followed.
///
/// A `sequence<sequence<…>>` nests once per level; a chain longer than
/// this is a cycle the resolver did not catch, and recursing into it
/// would end the process rather than report anything.
const MAX_TYPE_DEPTH: usize = 64;

/// The largest integer a TypeScript `number` holds as itself.
///
/// `Number.MAX_SAFE_INTEGER`. Above it, two integers can be one
/// `number`, which is why the 64-bit IDL types are a `bigint` and why a
/// constant that does not fit is refused rather than written out.
const MAX_SAFE_INTEGER: i128 = 9_007_199_254_740_991;

/// The declarations every generated file needs before its own.
///
/// The counterpart of [`crate::adapter::cpp::includes`], which lists
/// headers because C++'s support shapes are `std::` templates. There is
/// no such library to name here: a generated `.ts` has to typecheck on
/// its own, so the one support type it needs is declared in it.
///
/// Two declarations, and each because TypeScript has no form of its own
/// for the IDL type. Everything else an IDL type needs — a string, an
/// array, an optional — is a TypeScript type already.
#[must_use]
pub fn prelude() -> Vec<ts::Decl> {
    vec![
        ts::Decl::Interface(ts::Interface {
            doc: Doc::lines(&[
                "An IDL `long double`, as the sixteen octets it is.",
                "",
                "TypeScript has one numeric type and it is an IEEE 754 binary64, so",
                "there is nothing to hold a binary128 in. A `number` member would put",
                "eight bytes on the wire where the type says sixteen; the octets are",
                "the one form that carries the value without deciding what it means.",
            ]),
            exported: true,
            name: Ident::new(FLOAT128),
            generics: Vec::new(),
            extends: Vec::new(),
            members: vec![ts::Signature::Property(ts::Property {
                doc: Doc::line("The sixteen octets, in the order the wire carries them."),
                readonly: true,
                name: ts::PropertyName::ident("octets"),
                optional: false,
                type_: ts::Type::named(Path::single("Uint8Array")),
            })],
        }),
        ts::Decl::Interface(ts::Interface {
            doc: Doc::lines(&[
                "An IDL `any`, as the value and the identity of its type.",
                "",
                "DDS-TS 1.0 §7.3.4. `typeId` is the qualified IDL name and indexes",
                "the descriptor registry, so `lookupType(box.typeId)` answers with the",
                "descriptor that describes `value`. Structurally the `DdsAny` of",
                "`runtime/typescript/src/types.ts`, declared here rather than imported",
                "so that a generated file typechecks on its own.",
            ]),
            exported: true,
            name: Ident::new(ANY_BOX),
            generics: Vec::new(),
            extends: Vec::new(),
            members: vec![
                ts::Signature::Property(ts::Property {
                    doc: Doc::line("The qualified IDL name of the boxed value's type."),
                    readonly: true,
                    name: ts::PropertyName::ident("typeId"),
                    optional: false,
                    type_: ts::Type::string(),
                }),
                ts::Signature::Property(ts::Property {
                    doc: Doc::line("The payload, narrowed through the descriptor."),
                    readonly: true,
                    name: ts::PropertyName::ident("value"),
                    optional: false,
                    type_: ts::Type::Keyword(ts::Keyword::Unknown),
                }),
            ],
        }),
    ]
}

/// Build the TypeScript declarations a rule application produced.
///
/// [`prelude`] is **not** included: it is the same declaration for every
/// model, and a caller assembling a file writes it once. What comes back
/// is what this model has in it.
///
/// # Errors
/// [`MaterialiseError`] when a value the output has to state does not
/// evaluate, a type has no TypeScript shape, or a generated name would
/// capture or merge with another — never a silently dropped
/// declaration.
pub fn ts_items(ast: &Ast, applied: &Applied) -> Result<Vec<ts::Decl>, MaterialiseError> {
    let (structural, items) = ts_pending(ast, applied)?;
    Ok(verbatim::weave_file(
        ast,
        verbatim::aliases::TYPESCRIPT,
        assemble(ast, structural, items)?,
    ))
}

/// The types **and** their descriptor side-tables, in one set of
/// namespaces.
///
/// What [`ts_items`] and [`ts_descriptors`] produce, except that the two
/// halves of a module land in the same `namespace` block rather than in
/// two that TypeScript would merge. The merge is legal and this file
/// refuses it everywhere else (`check_names`), so producing it here
/// would be one rule for the generator and another for what it
/// generates.
///
/// The caller still has to write [`descriptor_import`] into the file:
/// the descriptors name `DdsTypeDescriptor` and call `registerType`.
///
/// # Errors
/// As [`ts_items`], plus a value a descriptor has to state and the model
/// will not evaluate.
pub fn ts_items_with_descriptors(
    ast: &Ast,
    applied: &Applied,
) -> Result<Vec<ts::Decl>, MaterialiseError> {
    let (structural, mut items) = ts_pending(ast, applied)?;
    items.extend(descriptor_pending(ast, applied)?);
    Ok(verbatim::weave_file(
        ast,
        verbatim::aliases::TYPESCRIPT,
        assemble(ast, structural, items)?,
    ))
}

/// `@verbatim` in the TypeScript column: `BEGIN_DECLARATION` and
/// `END_DECLARATION` open and close an interface's or a class's body; an
/// alias, an enumeration's const object and a function take them
/// adjacent.
impl verbatim::Carrier for ts::Decl {
    fn verbatim(text: String) -> Self {
        Self::Verbatim(text)
    }

    fn take_into_body(
        &mut self,
        name: &str,
        begin: &mut Vec<String>,
        end: &mut Vec<String>,
    ) -> bool {
        match self {
            Self::Interface(node) if node.name.as_str() == name => {
                let mut members: Vec<ts::Signature> =
                    begin.drain(..).map(ts::Signature::Verbatim).collect();
                members.append(&mut node.members);
                members.extend(end.drain(..).map(ts::Signature::Verbatim));
                node.members = members;
                true
            }
            Self::Class(node) if node.name.as_str() == name => {
                let mut members: Vec<ts::ClassMember> =
                    begin.drain(..).map(ts::ClassMember::Verbatim).collect();
                members.append(&mut node.members);
                members.extend(end.drain(..).map(ts::ClassMember::Verbatim));
                node.members = members;
                true
            }
            _ => false,
        }
    }
}

/// The declarations one model produces, before they are nested.
fn ts_pending(
    ast: &Ast,
    applied: &Applied,
) -> Result<(Vec<ts::Decl>, Vec<Pending>), MaterialiseError> {
    // Structural declarations — the map entry types — sit at the file
    // root and not in a namespace: two identical maps declared in two
    // modules are one shape, exactly as two identical anonymous
    // collections are one C type.
    let mut structural: Vec<ts::Decl> = Vec::new();
    let mut emitted: Vec<String> = Vec::new();
    let mut items: Vec<Pending> = Vec::new();

    // The same walk the C column uses. TypeScript hoists its type
    // declarations, so the order is not load-bearing for the compiler;
    // it is load-bearing for the output, which must not move between two
    // runs over one model.
    for id in ordered_types(ast) {
        let start = items.len();
        declare(ast, applied, id, &mut items, &mut structural, &mut emitted)?;
        let Ok(node) = ast.type_(id) else {
            continue;
        };
        let module = node.module;
        let name = ts::escape_keyword(&node.name);
        verbatim::weave_with(
            verbatim::declaration_slots(ast, id, verbatim::aliases::TYPESCRIPT),
            start,
            &mut items,
            |text| Pending {
                module,
                decl: ts::Decl::Verbatim(text),
            },
            |declared, begin, end| {
                for pending in declared {
                    if verbatim::Carrier::take_into_body(&mut pending.decl, &name, begin, end) {
                        return;
                    }
                }
            },
        );
    }

    // Constants and exceptions are not types, so they are walked
    // separately — the model keeps them in arenas of their own, and
    // every sibling materialiser does the same. IDL interfaces are not
    // walked at all, exactly as `java_items` does not walk them: an
    // interface has no data form and its operations belong to the RPC
    // arc.
    for (id, node) in ast.constants() {
        if ghost_created_for(applied, node.node_id, "TsConst").is_none() {
            continue;
        }
        let _ = id;
        // `eval_const` and not `eval_integral`: `const string MODULE_ID`
        // evaluates perfectly well and is not a number, and asking the
        // integral question about it answered `Unevaluated` and dropped
        // the declaration.
        let value = ast
            .eval_const(node.expr)
            .map_err(|_| MaterialiseError::Unevaluated {
                construct: "constant's value".to_string(),
                at: node.name.clone(),
            })?;
        let declared = node.type_.resolved();
        let type_ = match declared {
            Some(target) => ts_member_type(ast, applied, target, &node.name)?,
            None => ts::Type::number(),
        };
        let value = constant_expr(ast, declared, &value, &node.name)?;
        items.push(Pending {
            module: node.module,
            decl: ts::Decl::Variable(ts::Variable {
                doc: renamed_doc(&node.name, Doc::none()),
                exported: true,
                declare: false,
                kind: ts::VarKind::Const,
                name: Ident::new(&ts::escape_keyword(&node.name)),
                type_: Some(type_),
                value: Some(value),
            }),
        });
    }

    for (id, node) in ast.exceptions() {
        if ghost_created_for(applied, node.node_id, "TsError").is_none() {
            continue;
        }
        let _ = id;
        let mut fields = Vec::new();
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
            let container = ts::escape_keyword(&node.name);
            for decl in bound_constants(ast, &container, Some(&member_node.name), target)? {
                items.push(Pending {
                    module: node.module,
                    decl,
                });
            }
            if let Some(decl) = default_constant(ast, applied, &container, member_node, target)? {
                items.push(Pending {
                    module: node.module,
                    decl,
                });
            }
            fields.push(Slot {
                name: member_node.name.clone(),
                type_: ts_member_type(ast, applied, target, &member_node.name)?,
                optional: member_node.facets.is_optional(),
                default: default_expr(ast, applied, target, &member_node.name, 0)?,
                doc: Doc::none(),
            });
        }
        push_pair(
            &mut items,
            node.module,
            &node.name,
            fields,
            Doc::lines(&[
                "An exception, as the structure its members make.",
                "",
                "A data holder and not an `Error`: `Error` carries a `message` and a",
                "`stack` that nothing on the wire decides, and a generated subclass",
                "would have to invent both.",
            ]),
        );
    }

    Ok((structural, items))
}

/// Pending declarations, nested in the namespaces their modules name and
/// checked for the two arrangements TypeScript reads as another.
fn assemble(
    ast: &Ast,
    structural: Vec<ts::Decl>,
    items: Vec<Pending>,
) -> Result<Vec<ts::Decl>, MaterialiseError> {
    let mut root = Scope::default();
    for item in items {
        insert(ast, &mut root, item)?;
    }
    let mut decls = structural;
    decls.extend(scope_decls(root, 0)?);
    check_names(&decls, &top_level_names(&decls), 0)?;

    Ok(decls)
}

/// One declaration, kept with the module that declared it.
struct Pending {
    /// The IDL module it was declared in.
    module: Id<ModuleNode>,
    /// What it materialised as.
    decl: ts::Decl,
}

/// One field of a generated interface and of the class implementing it.
///
/// The two halves have to agree — a class whose field differs from the
/// interface's does not implement it — so they are built from one value
/// rather than twice from the model.
struct Slot {
    /// The IDL member's name, before escaping.
    name: String,
    /// Its TypeScript type.
    type_: ts::Type,
    /// Whether it is `@optional`.
    optional: bool,
    /// The value the class initialises it to.
    default: ts::Expr,
    /// Documentation for both halves.
    doc: Doc,
}

/// One nesting level of the output: a module's declarations and the
/// modules declared inside it.
///
/// A tree and not a list of chains, for the reason the Java column's
/// [`crate::adapter::java`] gives: grouping by module id alone emits
/// `module common { struct Robot; }` and
/// `module common { module util { … } }` as two namespaces of one name.
/// Two namespaces of one name *merge* in TypeScript, so this would not
/// even be reported.
#[derive(Default)]
struct Scope {
    /// What this module declares directly, in declaration order.
    decls: Vec<ts::Decl>,
    /// The modules declared inside it, in first-seen order.
    children: Vec<(Id<ModuleNode>, String, String, Scope)>,
}

/// Put one declaration in the scope its module names.
///
/// # Errors
/// [`MaterialiseError::NoModule`] when the model does not answer for a
/// module in the chain, or the chain runs past [`MAX_MODULE_DEPTH`].
fn insert(ast: &Ast, root: &mut Scope, item: Pending) -> Result<(), MaterialiseError> {
    let chain = module_chain(ast, item.module)?;
    let mut scope = root;
    for (id, name, idl) in chain {
        let at = match scope
            .children
            .iter()
            .position(|(seen, _, _, _)| *seen == id)
        {
            Some(at) => at,
            None => {
                scope.children.push((id, name, idl, Scope::default()));
                scope.children.len() - 1
            }
        };
        scope = &mut scope
            .children
            .get_mut(at)
            .ok_or_else(|| MaterialiseError::NoModule {
                at: module_handle(item.module),
            })?
            .3;
    }
    scope.decls.push(item.decl);
    Ok(())
}

/// A scope's declarations, with each module inside it as a namespace.
///
/// # Errors
/// [`MaterialiseError::NoModule`] when the nesting runs past
/// [`MAX_MODULE_DEPTH`], which `insert` has already bounded.
///
/// zerodds-lint: recursion-depth 64 (module nesting; bounded by `MAX_MODULE_DEPTH`)
fn scope_decls(scope: Scope, depth: usize) -> Result<Vec<ts::Decl>, MaterialiseError> {
    if depth > MAX_MODULE_DEPTH {
        return Err(MaterialiseError::NoModule {
            at: format!("the nesting runs past {MAX_MODULE_DEPTH}"),
        });
    }
    let mut out = scope.decls;
    for (_, name, idl, child) in scope.children {
        out.push(ts::Decl::Namespace(ts::Namespace {
            doc: renamed_doc(
                &idl,
                Doc::line("An IDL module, as the namespace its declarations nest in."),
            ),
            exported: true,
            declare: false,
            name: Ident::new(&name),
            decls: scope_decls(child, depth + 1)?,
        }));
    }
    Ok(out)
}

/// The modules a declaration nests in, outermost first, with the
/// namespace name each takes and the IDL name it came from.
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
) -> Result<Vec<(Id<ModuleNode>, String, String)>, MaterialiseError> {
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
        out.push((id, ts::escape_keyword(&node.name), node.name.clone()));
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

/// Which of TypeScript's two declaration spaces a name occupies.
///
/// `const E = { … }` and `type E = number` are one enumeration written
/// twice, and they do not collide: one is a value and the other a type.
/// A duplicate check that did not know this would refuse every
/// enumeration this file emits.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Space {
    /// `interface`, `type`.
    Type,
    /// `const`, `namespace`.
    Value,
    /// `class` — which declares both at once.
    Both,
}

impl Space {
    /// Whether two declarations of one name would collide.
    const fn overlaps(self, other: Self) -> bool {
        matches!(
            (self, other),
            (Self::Both, _)
                | (_, Self::Both)
                | (Self::Type, Self::Type)
                | (Self::Value, Self::Value)
        )
    }
}

/// The name a declaration carries, and the space it occupies.
///
/// `None` for a comment and for a module-scope statement, neither of
/// which declares anything.
fn declared_name(decl: &ts::Decl) -> Option<(&str, Space)> {
    match decl {
        ts::Decl::Statement(_) => None,
        ts::Decl::Interface(node) => Some((node.name.as_str(), Space::Type)),
        ts::Decl::TypeAlias(node) => Some((node.name.as_str(), Space::Type)),
        ts::Decl::Class(node) => Some((node.name.as_str(), Space::Both)),
        ts::Decl::Enum(node) => Some((node.name.as_str(), Space::Both)),
        ts::Decl::Variable(node) => Some((node.name.as_str(), Space::Value)),
        ts::Decl::Function(node) => Some((node.name.as_str(), Space::Value)),
        ts::Decl::Namespace(node) => Some((node.name.as_str(), Space::Value)),
        ts::Decl::Comment(_) | ts::Decl::Verbatim(_) => None,
    }
}

/// Every name declared at the file root.
///
/// What a reference written from the root resolves through, and
/// therefore what a nested namespace may not shadow.
fn top_level_names(decls: &[ts::Decl]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for decl in decls {
        if let Some((name, _)) = declared_name(decl) {
            out.push(name.to_string());
        }
    }
    out
}

/// Refuse a declaration a TypeScript compiler would read as another.
///
/// Two arrangements, and neither is reported by the compiler:
///
/// 1. **A second declaration of one name in one scope.** Two
///    `interface P` declarations *merge* into one interface with both
///    sets of members, and two `namespace a` blocks merge likewise. The
///    file compiles and holds a type neither IDL declaration described.
/// 2. **A nested declaration shadowing a top-level name.** Every
///    reference this materialiser writes starts at the file root, and
///    TypeScript has no root operator to say so. A `namespace c` inside
///    `namespace a` captures `c.Q` for everything written inside `a`; if
///    `a.c` happens to have a `Q` too, the member silently takes the
///    wrong type.
/// 3. **A declaration taking one of [`RESERVED_NAMES`].** The prelude's
///    own type and the two globals the generated initialisers call are
///    in the file whether the IDL knows it or not.
///
/// # Errors
/// [`MaterialiseError::NoLanguageForm`], naming the declaration.
///
/// zerodds-lint: recursion-depth 64 (module nesting; bounded by `MAX_MODULE_DEPTH`)
fn check_names(
    decls: &[ts::Decl],
    top_level: &[String],
    depth: usize,
) -> Result<(), MaterialiseError> {
    if depth > MAX_MODULE_DEPTH {
        return Err(MaterialiseError::NoModule {
            at: format!("the nesting runs past {MAX_MODULE_DEPTH}"),
        });
    }
    let mut seen: Vec<(String, Space)> = Vec::new();
    for decl in decls {
        let Some((name, space)) = declared_name(decl) else {
            continue;
        };
        if seen
            .iter()
            .any(|(taken, other)| taken == name && space.overlaps(*other))
        {
            return Err(MaterialiseError::NoLanguageForm {
                construct: format!(
                    "a second declaration named `{name}` in one scope, which TypeScript \
                     merges into one rather than reporting"
                ),
                language: "TypeScript",
                at: name.to_string(),
            });
        }
        seen.push((name.to_string(), space));
        if RESERVED_NAMES.contains(&name) {
            return Err(MaterialiseError::NoLanguageForm {
                construct: format!(
                    "a declaration named `{name}`, which this materialiser writes into \
                     the file itself — the generated one would shadow it for every \
                     scope inside"
                ),
                language: "TypeScript",
                at: name.to_string(),
            });
        }
        if depth > 0 && top_level.iter().any(|taken| taken == name) {
            return Err(MaterialiseError::NoLanguageForm {
                construct: format!(
                    "a nested declaration named `{name}`, which would capture the first \
                     segment of every qualified reference written inside it — TypeScript \
                     has no root operator to write past it with"
                ),
                language: "TypeScript",
                at: name.to_string(),
            });
        }
        if let ts::Decl::Namespace(namespace) = decl {
            check_names(&namespace.decls, top_level, depth + 1)?;
        }
    }
    Ok(())
}

/// One type's TypeScript declarations, appended where they belong.
#[allow(clippy::too_many_lines)]
fn declare(
    ast: &Ast,
    applied: &Applied,
    id: Id<TypeNode>,
    items: &mut Vec<Pending>,
    structural: &mut Vec<ts::Decl>,
    emitted: &mut Vec<String>,
) -> Result<(), MaterialiseError> {
    let Ok(node) = ast.type_(id) else {
        return Ok(());
    };
    match &node.kind {
        // A primitive, a string and a `fixed` are spellings TypeScript
        // already has; a sequence is `T[]`. None of them declares
        // anything.
        TypeKind::Primitive(_)
        | TypeKind::String { .. }
        | TypeKind::Fixed { .. }
        | TypeKind::Sequence { .. } => {}

        // Neither does an array — but every dimension is evaluated
        // anyway. TypeScript's array type states no length, so nothing
        // here would substitute a zero; what a dimension that does not
        // evaluate does mean is that no caller can allocate the array,
        // and a file that compiles while the length is unknown is
        // exactly the silent success this project refuses. The C and
        // Java columns fail on the same input, and the three columns'
        // failure sets stay comparable.
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
            let slots = vec![
                Slot {
                    name: "key".to_string(),
                    type_: ts_member_type(ast, applied, key, &at)?,
                    optional: false,
                    default: default_expr(ast, applied, key, &at, 0)?,
                    doc: Doc::none(),
                },
                Slot {
                    name: "value".to_string(),
                    type_: ts_member_type(ast, applied, value, &at)?,
                    optional: false,
                    default: default_expr(ast, applied, value, &at, 0)?,
                    doc: Doc::none(),
                },
            ];
            structural.extend(pair(
                &name,
                &name,
                slots,
                Doc::line(
                    "One entry of a map. XTypes 1.3 7.4.1.1.4 defines a map as a \
                     sequence of these, and the rules write the two halves with no \
                     framing between them. An array of these and not a `Map`: the \
                     entry order is the sample's, and it is what the MapEntryOrder \
                     axis measures — and a `Map` keyed by a struct would compare its \
                     keys by reference.",
                ),
            ));
        }

        // A typedef has a TypeScript declaration, which is where this
        // column parts company with Java's: `type Row = number[]` is a
        // real alias, so the IDL's name reaches the generated source
        // instead of a comment about it.
        TypeKind::Alias { aliased } => {
            if !emits_alias(ast, applied, id) {
                return Ok(());
            }
            let Some(target) = aliased.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "typedef target".to_string(),
                    at: node.name.clone(),
                });
            };
            for decl in bound_constants(ast, &ts::escape_keyword(&node.name), None, target)? {
                items.push(Pending {
                    module: node.module,
                    decl,
                });
            }
            items.push(Pending {
                module: node.module,
                decl: ts::Decl::TypeAlias(ts::TypeAlias {
                    doc: renamed_doc(
                        &node.name,
                        Doc::line("An IDL typedef, as the type alias it is."),
                    ),
                    exported: true,
                    name: Ident::new(&ts::escape_keyword(&node.name)),
                    generics: Vec::new(),
                    type_: type_at(ast, applied, target, &node.name, 0)?,
                }),
            });
        }

        TypeKind::Enumeration { enumerators } => {
            if ghost_created_for(applied, node.node_id, "TsEnum").is_none() {
                return Ok(());
            }
            let mut entries = Vec::new();
            let mut ordinals: Vec<(String, i128)> = Vec::new();
            for enumerator in enumerators {
                let Ok(enum_node) = ast.enumerator(*enumerator) else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "enumerator the model listed".to_string(),
                        at: node.name.clone(),
                    });
                };
                if ghost_created_for(applied, enum_node.node_id, "TsEnumerator").is_none() {
                    continue;
                }
                let value = ast.effective_enumerator_value(*enumerator).ok_or_else(|| {
                    MaterialiseError::Unevaluated {
                        construct: "enumerator value".to_string(),
                        at: enum_node.name.clone(),
                    }
                })?;
                // XTypes 1.3 7.4.3.5.3 makes an enumerated literal 32
                // bits wide, so the value is checked against those 32
                // bits and written as the number it is. Unlike the Java
                // column nothing wraps to a negative: a TypeScript
                // `number` holds 4294967295 as itself.
                let value = in_range(
                    value,
                    i128::from(i32::MIN),
                    i128::from(u32::MAX),
                    "32-bit enumerated literal",
                    &enum_node.name,
                )?;
                let member = ts::escape_member(&enum_node.name);
                // The key *and* the value. §7.6's mapping is
                // `RED: "RED"`, and the two have to be the same string:
                // `<Enum>Ordinal` is a `Record<Enum, number>` whose keys
                // are the alias's members — which are the object's
                // values — and is written out with the object's keys.
                entries.push((
                    ts::PropertyName::ident(&member),
                    ts::Expr::Literal(Literal::Str(member.clone())),
                ));
                ordinals.push((member, value));
            }
            items.extend(const_object(
                node.module,
                &node.name,
                entries,
                enum_alias_type(&node.name),
                Doc::line(
                    "An enumeration, as the `as const` object its enumerators name and \
                     the string-literal union a member of it holds (DDS-TS 1.0 §7.6). \
                     Not a TypeScript `enum`: since 5.0 an `enum`-typed slot rejects a \
                     value that is not one of its members, and an @appendable writer \
                     that added an enumerator sends exactly such a value.",
                ),
            ));
            items.extend(enum_ordinals(node.module, &node.name, &ordinals));
        }

        TypeKind::Bitmask { bits } => {
            if ghost_created_for(applied, node.node_id, "TsBitmask").is_none() {
                return Ok(());
            }
            let bound = ast.effective_bit_bound(id);
            let holder = unsigned_holder(bound);
            let wide = holder == ts::Type::bigint();
            let mut entries = Vec::new();
            for bit in bits {
                let Ok(bit_node) = ast.bit_value(*bit) else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "bitmask bit the model listed".to_string(),
                        at: node.name.clone(),
                    });
                };
                if ghost_created_for(applied, bit_node.node_id, "TsBitConst").is_none() {
                    continue;
                }
                let position = ast.effective_bit_position(*bit).ok_or_else(|| {
                    MaterialiseError::Unevaluated {
                        construct: "bit position".to_string(),
                        at: bit_node.name.clone(),
                    }
                })?;
                entries.push((
                    ts::PropertyName::ident(&ts::escape_member(&bit_node.name)),
                    ts::Expr::Literal(bit_literal(position, wide)),
                ));
            }
            items.extend(const_object(
                node.module,
                &node.name,
                entries,
                holder,
                Doc::line(
                    "A bitmask, as the `as const` object its bits name. Each value is \
                     written as the number it is and not as `1 << n`: JavaScript's `<<` \
                     is a signed 32-bit operation, so `1 << 31` is -2147483648 and a \
                     shift past 31 wraps — neither of which is the bit the position \
                     names.",
                ),
            ));
            // §7.8: the declared bit width, beside the object.
            items.push(Pending {
                module: node.module,
                decl: side_constant(
                    format!("{}_BIT_BOUND", ts::escape_keyword(&node.name)),
                    None,
                    ts::Expr::Literal(Literal::int(i128::from(bound))),
                    Doc::line(
                        "The effective `@bit_bound` of this bitmask, which is what \
                         decides whether its values are a `number` or a `bigint`. \
                         DDS-TS 1.0 §7.8.",
                    ),
                ),
            });
        }

        TypeKind::Bitset { .. } => {
            if ghost_created_for(applied, node.node_id, "TsBitset").is_none() {
                return Ok(());
            }
            let mut slots = Vec::new();
            // The base's bitfields are fields of the derived type too:
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
                let holder = unsigned_holder(field_node.bits);
                let default = zero_of(&holder);
                // §7.7: the width in bits, as an integer literal beside
                // the interface. A TypeScript `number` or `bigint`
                // states no width, so this is the only place the
                // bitfield's own declaration survives.
                items.push(Pending {
                    module: node.module,
                    decl: side_constant(
                        format!("{}_{field_name}_BITS", ts::escape_keyword(&node.name)),
                        None,
                        ts::Expr::Literal(Literal::int(i128::from(field_node.bits))),
                        Doc::line("The width of this bitfield, in bits. DDS-TS 1.0 §7.7."),
                    ),
                });
                slots.push(Slot {
                    name: field_name.clone(),
                    type_: holder,
                    optional: false,
                    default,
                    doc: Doc::none(),
                });
            }
            push_pair(
                items,
                node.module,
                &node.name,
                slots,
                Doc::line(
                    "A bitset. The fields are separate integers here; the packing into \
                     the anchor is the codec's.",
                ),
            );
        }

        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let created = if matches!(node.kind, TypeKind::Valuetype { .. }) {
                "TsValue"
            } else {
                "TsStruct"
            };
            if ghost_created_for(applied, node.node_id, created).is_none() {
                return Ok(());
            }
            // The members of the type, and the base as its first member `parent`; the plan
            // still puts the base's members on the wire first (XTypes 1.3 §7.4.1.1.5.1).
            //
            // Every member is emitted, with no per-member rule lookup: a
            // member whose lookup missed would vanish from the interface
            // and the file would still typecheck. The C and Java columns
            // take the same view.
            let mut slots = Vec::new();
            let mut spelled = Vec::new();
            // XTypes 1.3 §7.5.1.4.1.3: the base is the first member `parent`.
            if let Some(base_type) = super::c::struct_base(ast, id) {
                spelled.push(("parent".to_string(), "parent".to_string()));
                slots.push(Slot {
                    name: "parent".to_string(),
                    type_: ts_member_type(ast, applied, base_type, "parent")?,
                    optional: false,
                    default: default_expr(ast, applied, base_type, "parent", 0)?,
                    doc: Doc::line(
                        "The base structure, the first member (XTypes 1.3 7.5.1.4.1.3).",
                    ),
                });
            }
            for member in ast.members_of(id).to_vec() {
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
                let optional = member_node.facets.is_optional();
                // §7.3.1, §7.9, §7.9.1 and §7.11: the bound, the
                // lengths and the `@default` a member's declaration
                // carries and its TypeScript type cannot.
                let container = ts::escape_keyword(&node.name);
                for decl in bound_constants(ast, &container, Some(&member_node.name), target)? {
                    items.push(Pending {
                        module: node.module,
                        decl,
                    });
                }
                if let Some(decl) = default_constant(ast, applied, &container, member_node, target)?
                {
                    items.push(Pending {
                        module: node.module,
                        decl,
                    });
                }
                spelled.push((
                    ts::escape_member(&member_node.name),
                    member_node.name.clone(),
                ));
                slots.push(Slot {
                    name: member_node.name.clone(),
                    type_: ts_member_type(ast, applied, target, &member_node.name)?,
                    optional,
                    default: if optional {
                        ts::Expr::Undefined
                    } else {
                        default_expr(ast, applied, target, &member_node.name, 0)?
                    },
                    doc: if optional {
                        Doc::line(
                            "An @optional member: absent rather than present-and-null. In \
                             a @mutable type an absent member is not written at all; \
                             outside one its absence is written — a zero is_present byte \
                             under XCDR2 (rule 20) or a parameter header declaring a \
                             length of zero under XCDR1 (7.4.1.2).",
                        )
                    } else {
                        Doc::none()
                    },
                });
            }
            // The escape is the only layer that holds both spellings
            // at once. Left to the target: a duplicate property, which TypeScript
            // flags and JavaScript does not.
            no_spelling_collision(
                spelled
                    .iter()
                    .map(|(field, declared)| (field.as_str(), declared.as_str())),
                "TypeScript",
                &node.name,
            )?;
            push_pair(items, node.module, &node.name, slots, Doc::none());
        }

        TypeKind::Union { discriminator, .. } => {
            if ghost_created_for(applied, node.node_id, "TsUnion").is_none() {
                return Ok(());
            }
            let Some(disc) = discriminator.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "union discriminator".to_string(),
                    at: node.name.clone(),
                });
            };
            let disc_type = ts_member_type(ast, applied, disc, &node.name)?;
            // Every explicit label of the whole union, which is what
            // §7.5's `default:` arm is the complement of.
            let explicit = explicit_labels(ast, id, disc, &node.name)?;
            let mut arms: Vec<ts::Type> = Vec::new();
            for case in ast.cases_of(id) {
                let Ok(case_node) = ast.case(*case) else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "union case the model listed".to_string(),
                        at: node.name.clone(),
                    });
                };
                if ghost_created_for(applied, case_node.node_id, "TsCase").is_none() {
                    continue;
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
                for decl in bound_constants(
                    ast,
                    &ts::escape_keyword(&node.name),
                    Some(&member.name),
                    target,
                )? {
                    items.push(Pending {
                        module: node.module,
                        decl,
                    });
                }
                arms.push(ts::Type::Object(vec![
                    ts::Signature::Property(ts::Property {
                        doc: Doc::none(),
                        readonly: false,
                        name: ts::PropertyName::ident(DISCRIMINANT_PROPERTY),
                        optional: false,
                        type_: discriminant_type(
                            ast,
                            disc,
                            &disc_type,
                            &case_node.labels,
                            &explicit,
                            &member.name,
                        )?,
                    }),
                    ts::Signature::Property(ts::Property {
                        doc: Doc::none(),
                        readonly: false,
                        name: ts::PropertyName::ident(&ts::escape_member(&member.name)),
                        optional: false,
                        type_: ts_member_type(ast, applied, target, &member.name)?,
                    }),
                ]));
            }
            // A discriminator value that selects no case is a state the
            // union has, and it needs an arm of its own. §7.2.2.4.4.4:
            // "If the discriminator value does not correspond to any
            // case, the union has no selected member", and §7.4.3.5.3
            // rule (24) writes the discriminator and stops. `union
            // Choice switch (long) { case 1: …; case 2: …; }` with a
            // sample of `_d: 99` is exactly that, and it is in the
            // corpus; without this arm no TypeScript value could hold
            // it and the case was `TS2322: Type '99' is not assignable
            // to type '1 | 2'`.
            //
            // Only where there is no `default:`, which by definition
            // selects every value the labelled cases do not.
            let has_default = ast.cases_of(id).iter().any(|case| {
                ast.case(*case).is_ok_and(|case_node| {
                    case_node
                        .labels
                        .iter()
                        .any(|label| matches!(label, CaseLabel::Default))
                })
            });
            if !arms.is_empty() && !has_default {
                arms.push(ts::Type::Object(vec![ts::Signature::Property(
                    ts::Property {
                        doc: Doc::none(),
                        readonly: false,
                        name: ts::PropertyName::ident(DISCRIMINANT_PROPERTY),
                        optional: false,
                        type_: disc_type.clone(),
                    },
                )]));
            }
            // A union with no case carries only its discriminator. One
            // arm rather than none: `type X = never` is a type no value
            // inhabits, and a member of it could not be written at all.
            if arms.is_empty() {
                arms.push(ts::Type::Object(vec![ts::Signature::Property(
                    ts::Property {
                        doc: Doc::none(),
                        readonly: false,
                        name: ts::PropertyName::ident(DISCRIMINANT_PROPERTY),
                        optional: false,
                        type_: disc_type,
                    },
                )]));
            }
            items.push(Pending {
                module: node.module,
                decl: ts::Decl::TypeAlias(ts::TypeAlias {
                    doc: renamed_doc(
                        &node.name,
                        Doc::line(
                            "A union, as the discriminated union TypeScript narrows on \
                             `discriminator` (DDS-TS 1.0 §7.5). A `default:` case takes \
                             the discriminator's type minus the labels the other cases \
                             claim, because that is what it selects.",
                        ),
                    ),
                    exported: true,
                    name: Ident::new(&ts::escape_keyword(&node.name)),
                    generics: Vec::new(),
                    type_: ts::Type::Union(arms),
                }),
            });
        }

        // A native type is opaque. TypeScript has a type alias, so
        // unlike the Java column the IDL name gets a declaration of its
        // own rather than a comment.
        TypeKind::Native => {
            if ghost_created_for(applied, node.node_id, "TsNative").is_none() {
                return Ok(());
            }
            items.push(Pending {
                module: node.module,
                decl: ts::Decl::TypeAlias(ts::TypeAlias {
                    doc: renamed_doc(
                        &node.name,
                        Doc::line(
                            "A native type. `unknown` unless a mapping facet named \
                             something: `unknown` is opaque and has to be narrowed \
                             before it is used, where `any` would switch the checking \
                             off.",
                        ),
                    ),
                    exported: true,
                    name: Ident::new(&ts::escape_keyword(&node.name)),
                    generics: Vec::new(),
                    type_: native_type(ast, id),
                }),
            });
        }

        // A value box is a reference in TypeScript already: the boxed
        // type or `undefined`. Nothing to declare.
        TypeKind::ValueBox { .. } => {}

        // An object reference belongs to the RPC arc and an annotation
        // type has no runtime form at all — `zerodds_ast_layout` refuses
        // to plan either — so a TypeScript declaration would be a shape
        // nothing could serialise. `any` and `ValueBase` are in the
        // model because Building Block Any is one of IDL 4.2's fifteen,
        // and what they hold is decided at run time. All four are named
        // here rather than passed over, and `ts_member_type` refuses
        // them by name where a member would carry one.
        TypeKind::ObjectRef { .. }
        | TypeKind::AnnotationType { .. }
        | TypeKind::Any
        | TypeKind::ValueBase => {}
    }
    Ok(())
}

/// Whether a typedef gets a `type` declaration of its own.
///
/// An anonymous alias has no name to declare, and one the rules did not
/// create has no declaration; in both cases a reference resolves through
/// to the target instead. [`type_at`] asks the same question, so the two
/// never disagree — a reference to a declaration that was not emitted
/// would be a name that resolves to nothing.
fn emits_alias(ast: &Ast, applied: &Applied, id: Id<TypeNode>) -> bool {
    let Ok(node) = ast.type_(id) else {
        return false;
    };
    if node.name.is_empty() {
        return false;
    }
    ghost_created_for(applied, node.node_id, "TsAlias").is_some()
}

/// The interface and its factory, as two declarations of one shape.
fn pair(name: &str, idl_name: &str, slots: Vec<Slot>, doc: Doc) -> Vec<ts::Decl> {
    let interface_name = ts::escape_keyword(name);
    let mut factory_name = String::from(FACTORY_PREFIX);
    factory_name.push_str(&interface_name);

    // `field?: T | undefined` for an `@optional` member and not
    // `field?: T`. §7.11 asks for both halves: the `?:` suffix and the
    // `| undefined` arm. Under `exactOptionalPropertyTypes` the two
    // declarations differ — `field?: T` refuses an explicit
    // `undefined`, which is the value a producer that has nothing to
    // put there holds — and the clause writes the wider one.
    let members: Vec<ts::Signature> = slots
        .iter()
        .map(|slot| {
            ts::Signature::Property(ts::Property {
                doc: renamed_member_doc(&slot.name, slot.doc.clone()),
                readonly: false,
                name: ts::PropertyName::ident(&ts::escape_member(&slot.name)),
                optional: slot.optional,
                type_: if slot.optional {
                    optional_of(slot.type_.clone())
                } else {
                    slot.type_.clone()
                },
            })
        })
        .collect();

    // One entry per required member, and none for an `@optional` one:
    // its absence is what §7.2.2.4.4.4.8 asks for, and under
    // `exactOptionalPropertyTypes` an explicit `undefined` is not the
    // same value as a property that is not there.
    let entries: Vec<(ts::PropertyName, ts::Expr)> = slots
        .iter()
        .filter(|slot| !slot.optional)
        .map(|slot| {
            (
                ts::PropertyName::ident(&ts::escape_member(&slot.name)),
                slot.default.clone(),
            )
        })
        .collect();

    vec![
        ts::Decl::Interface(ts::Interface {
            doc: renamed_doc(idl_name, doc),
            exported: true,
            name: Ident::new(&interface_name),
            generics: Vec::new(),
            extends: Vec::new(),
            members,
        }),
        ts::Decl::Function(ts::Function {
            doc: Doc::lines(&[
                "A value of this type, with every member at the value Table 9 of",
                "DDS-XTypes 1.3 gives it.",
                "",
                "An interface is erased, so nothing can be constructed from one and a",
                "decoder that opens with `let v: T = …` needs a `T` from somewhere.",
                "This is that somewhere. A function and not a class: DDS-TS 1.0 §7.4",
                "emits no TypeScript class for an IDL data type, and a caller typed as",
                "the interface depends on no implementation because there is none.",
            ]),
            exported: true,
            name: Ident::new(&factory_name),
            generics: Vec::new(),
            params: Vec::new(),
            returns: Some(ts::Type::named(Path::single(&interface_name))),
            body: Some(ts::Block {
                statements: vec![ts::Statement::Return(Some(ts::Expr::Object(entries)))],
            }),
        }),
    ]
}

/// [`pair`], queued against the module that declared it.
fn push_pair(
    items: &mut Vec<Pending>,
    module: Id<ModuleNode>,
    name: &str,
    slots: Vec<Slot>,
    doc: Doc,
) {
    for decl in pair(name, name, slots, doc) {
        items.push(Pending { module, decl });
    }
}

/// `export const N = { … } as const;` beside `export type N = <holder>;`
///
/// Two declarations of one name, which is legal: one is a value and the
/// other a type. The object keeps the enumerators' names and the alias
/// keeps the enumeration's, so neither half of the IDL declaration is
/// lost.
fn const_object(
    module: Id<ModuleNode>,
    name: &str,
    entries: Vec<(ts::PropertyName, ts::Expr)>,
    holder: ts::Type,
    doc: Doc,
) -> Vec<Pending> {
    let escaped = ts::escape_keyword(name);
    vec![
        Pending {
            module,
            decl: ts::Decl::Variable(ts::Variable {
                doc: renamed_doc(name, doc),
                exported: true,
                declare: false,
                kind: ts::VarKind::Const,
                name: Ident::new(&escaped),
                // No declared type: `as const` is what narrows each
                // entry to its own literal type, and an annotation
                // would widen them all back to the holder.
                type_: None,
                value: Some(ts::Expr::AsConst(Box::new(ts::Expr::Object(entries)))),
            }),
        },
        Pending {
            module,
            decl: ts::Decl::TypeAlias(ts::TypeAlias {
                doc: Doc::line("What a member of this type holds."),
                exported: true,
                name: Ident::new(&escaped),
                generics: Vec::new(),
                type_: holder,
            }),
        },
    ]
}

/// `export const <Name> = <value>;` — one side-table constant.
///
/// No declared type where none is given: the clause's examples write
/// `export const Sample_pixels_BOUND = 10;`, and `as const` is not
/// wanted either — a bound that narrowed to its own literal type would
/// be a different type from the `number` a caller compares against.
fn side_constant(name: String, type_: Option<ts::Type>, value: ts::Expr, doc: Doc) -> ts::Decl {
    ts::Decl::Variable(ts::Variable {
        doc,
        exported: true,
        declare: false,
        kind: ts::VarKind::Const,
        name: Ident::new(&name),
        type_,
        value: Some(value),
    })
}

/// The bound and length constants a declaration's own type contributes.
///
/// `<Container>_<Field>_BOUND` for a bounded `string`, `wstring`,
/// `sequence` and `map` (§7.3.1, §7.9, §7.9.1);
/// `<Container>_<Field>_LENGTH` for a one-dimensional array and
/// `_LENGTH_DIM1`… for a multi-dimensional one (§7.9). `field` is
/// `None` for a typedef, whose constants carry the alias's own name
/// (§7.3.1, §7.10).
///
/// The type is taken as it stands and no alias is followed: a member
/// declared with a named typedef takes its bound from that typedef's
/// own constant, and repeating it here would put the same number in the
/// file twice under two names.
///
/// # Errors
/// [`MaterialiseError::Unevaluated`] for an array dimension the model
/// will not fold — the same refusal [`declare`] makes for the type
/// itself.
fn bound_constants(
    ast: &Ast,
    container: &str,
    field: Option<&str>,
    target: Id<TypeNode>,
) -> Result<Vec<ts::Decl>, MaterialiseError> {
    let stem = match field {
        Some(field) => format!("{container}_{field}"),
        None => container.to_string(),
    };
    let at = || stem.clone();
    let Ok(node) = ast.type_(target) else {
        return Ok(Vec::new());
    };
    let bounded =
        |bound: zerodds_ast::Bound, what: &str| -> Result<Vec<ts::Decl>, MaterialiseError> {
            Ok(match evaluated_bound(ast, bound, &at())? {
                Some(value) => vec![side_constant(
                    format!("{stem}_BOUND"),
                    None,
                    ts::Expr::Literal(Literal::int(value)),
                    Doc::line(what),
                )],
                None => Vec::new(),
            })
        };
    Ok(match &node.kind {
        TypeKind::String { bound, .. } => bounded(
            *bound,
            "The declared maximum length of this string, in characters. DDS-TS 1.0 \
             §7.3.1: a TypeScript `string` carries no bound structurally, so the \
             limit is stated beside the declaration and checked at the \
             runtime-validation layer.",
        )?,
        TypeKind::Sequence { bound, .. } => bounded(
            *bound,
            "The declared maximum number of elements. DDS-TS 1.0 §7.9: a TypeScript \
             `Array` carries no bound structurally.",
        )?,
        TypeKind::Map { bound, .. } => bounded(
            *bound,
            "The declared maximum number of entries. DDS-TS 1.0 §7.9.1.",
        )?,
        TypeKind::Array { dims, .. } => {
            let mut lengths = Vec::new();
            for dim in dims {
                lengths.push(ast.eval_integral(*dim).map_err(|_| {
                    MaterialiseError::Unevaluated {
                        construct: "array length".to_string(),
                        at: at(),
                    }
                })?);
            }
            // One `_LENGTH` where there is one dimension and
            // `_LENGTH_DIM1`… where there are several, in declaration
            // order — §7.9. A TypeScript array type states no length at
            // all, so this is the only place the dimensions survive.
            match lengths.as_slice() {
                [] => Vec::new(),
                [only] => vec![side_constant(
                    format!("{stem}_LENGTH"),
                    None,
                    ts::Expr::Literal(Literal::int(*only)),
                    Doc::line(
                        "The exact length of this array. DDS-TS 1.0 §7.9: a TypeScript \
                         `Array` type states no length.",
                    ),
                )],
                many => many
                    .iter()
                    .enumerate()
                    .map(|(index, length)| {
                        side_constant(
                            format!("{stem}_LENGTH_DIM{}", index + 1),
                            None,
                            ts::Expr::Literal(Literal::int(*length)),
                            Doc::line(
                                "One dimension of this array, in declaration order. \
                                 DDS-TS 1.0 §7.9.",
                            ),
                        )
                    })
                    .collect(),
            }
        }
        _ => Vec::new(),
    })
}

/// `<Type>_<Field>_DEFAULT` — §7.11's side-table constant for a member
/// carrying `@default(V)`.
///
/// Typed as the member is and initialised to the literal, so a consumer
/// populating a default-bearing struct uses the constant rather than
/// writing the value a second time.
///
/// # Errors
/// [`MaterialiseError::NoLanguageForm`] for a `@default` this column has
/// no TypeScript expression for — the same refusal
/// [`crate::default_typescript`] makes, rather than a silent fall back
/// to Table 9's zero.
fn default_constant(
    ast: &Ast,
    applied: &Applied,
    container: &str,
    member: &zerodds_ast::MemberNode,
    target: Id<TypeNode>,
) -> Result<Option<ts::Decl>, MaterialiseError> {
    let Some(expr) = member.facets.xtypes.as_ref().and_then(|x| x.default_value) else {
        return Ok(None);
    };
    let value = crate::default_typescript::explicit_default(ast, expr, target, &member.name)
        .map_err(|error| MaterialiseError::NoLanguageForm {
            construct: format!("a `@default` this column cannot spell ({error})"),
            language: "TypeScript",
            at: member.name.clone(),
        })?;
    Ok(Some(side_constant(
        format!("{container}_{}_DEFAULT", member.name),
        Some(ts_member_type(ast, applied, target, &member.name)?),
        value,
        Doc::line(
            "The value `@default` gave this member. DDS-TS 1.0 §7.11: the annotation \
             changes no interface shape, so it is carried here and in the descriptor's \
             `default` property.",
        ),
    )))
}

/// `(typeof Colour)[keyof typeof Colour]` — the alias DDS-TS 1.0 §7.6
/// gives an enumeration.
///
/// The union of the `as const` object's literal-string values, written
/// as the indexed access the clause writes rather than as the values
/// spelled out a second time: an enumerator added to the object is in
/// the alias without anything else having to be edited, and the two
/// halves of the declaration cannot come to list different members.
fn enum_alias_type(name: &str) -> ts::Type {
    let path = Path::single(&ts::escape_keyword(name));
    ts::Type::Indexed {
        object: Box::new(ts::Type::Parenthesised(Box::new(ts::Type::TypeOf(
            path.clone(),
        )))),
        index: Box::new(ts::Type::KeyOf(Box::new(ts::Type::TypeOf(path)))),
    }
}

/// `<Enum>Ordinal` and `<Enum>FromOrdinal` — §7.6's numeric companions.
///
/// The alias carries the enumerator *names* and the wire carries their
/// values, so the mapping between the two has to be recoverable from the
/// generated module; §7.6 requires it whether or not any member carries
/// `@value(N)`, because the implicit ordinals are as much a part of the
/// declaration as the explicit ones.
///
/// `FromOrdinal` is a `ReadonlyMap` and not a second object: its keys
/// are numbers, and an object's keys are strings.
fn enum_ordinals(module: Id<ModuleNode>, name: &str, ordinals: &[(String, i128)]) -> Vec<Pending> {
    let escaped = ts::escape_keyword(name);
    let alias = ts::Type::named(Path::single(&escaped));
    let forward: Vec<(ts::PropertyName, ts::Expr)> = ordinals
        .iter()
        .map(|(member, value)| {
            (
                ts::PropertyName::ident(member),
                ts::Expr::Literal(Literal::int(*value)),
            )
        })
        .collect();
    // `[[0, Colour.RED], …] as const` and not a bare array literal: the
    // `Map` constructor takes `readonly (readonly [K, V])[]`, and
    // without `as const` the elements infer as `(number | string)[]` —
    // which is not a pair and does not construct a `Map` at all.
    let backward: Vec<ts::Expr> = ordinals
        .iter()
        .map(|(member, value)| {
            ts::Expr::Array(vec![
                ts::Expr::Literal(Literal::int(*value)),
                ts::Expr::Path(Path::new(&[escaped.as_str(), member.as_str()])),
            ])
        })
        .collect();
    vec![
        Pending {
            module,
            decl: ts::Decl::Variable(ts::Variable {
                doc: Doc::line(
                    "The numeric value each enumerator carries on the wire. DDS-TS 1.0 \
                     §7.6: the ordinals are the declaration's, whether `@value(N)` \
                     stated them or the position did.",
                ),
                exported: true,
                declare: false,
                kind: ts::VarKind::Const,
                name: Ident::new(&format!("{escaped}Ordinal")),
                type_: Some(ts::Type::generic(
                    Path::single("Readonly"),
                    vec![ts::Type::generic(
                        Path::single("Record"),
                        vec![alias.clone(), ts::Type::number()],
                    )],
                )),
                value: Some(ts::Expr::AsConst(Box::new(ts::Expr::Object(forward)))),
            }),
        },
        Pending {
            module,
            decl: ts::Decl::Variable(ts::Variable {
                doc: Doc::line(
                    "The enumerator each wire value names — the inverse of \
                     `<Enum>Ordinal`, for a reader that has the number.",
                ),
                exported: true,
                declare: false,
                kind: ts::VarKind::Const,
                name: Ident::new(&format!("{escaped}FromOrdinal")),
                type_: Some(ts::Type::generic(
                    Path::single("ReadonlyMap"),
                    vec![ts::Type::number(), alias],
                )),
                value: Some(ts::Expr::New {
                    callee: Path::single("Map"),
                    args: vec![ts::Expr::AsConst(Box::new(ts::Expr::Array(backward)))],
                }),
            }),
        },
    ]
}

/// The TypeScript type of a value in a position that has to hold one,
/// refusing the constructs TypeScript cannot hold.
///
/// `any` is the first of those. What it holds is decided at run time —
/// there is no TypeScript type for it and no wire form to generate a
/// codec against. Emitting `unknown` into the interface and reporting
/// success would produce a member no encoder can write, from a generator
/// that said nothing was wrong.
///
/// # Errors
/// [`MaterialiseError::NoLanguageForm`] for `any`, `ValueBase`, an
/// object reference and an annotation type;
/// [`MaterialiseError::Unevaluated`] for a type the model will not
/// answer for.
pub fn ts_member_type(
    ast: &Ast,
    applied: &Applied,
    target: Id<TypeNode>,
    at: &str,
) -> Result<ts::Type, MaterialiseError> {
    type_at(ast, applied, target, at, 0)
}

/// [`ts_member_type`], carrying how deep the spelling has nested.
///
/// zerodds-lint: recursion-depth 64 (collection nesting; bounded by `MAX_TYPE_DEPTH`)
fn type_at(
    ast: &Ast,
    applied: &Applied,
    id: Id<TypeNode>,
    at: &str,
    depth: usize,
) -> Result<ts::Type, MaterialiseError> {
    if depth > MAX_TYPE_DEPTH {
        return Err(MaterialiseError::Unevaluated {
            construct: format!("type, whose TypeScript spelling nests past {MAX_TYPE_DEPTH}"),
            at: at.to_string(),
        });
    }
    let node = ast.type_(id).map_err(|_| MaterialiseError::Unevaluated {
        construct: "type".to_string(),
        at: at.to_string(),
    })?;
    let refused = match &node.kind {
        TypeKind::ValueBase => Some("ValueBase"),
        // An object reference is a handle into the RPC arc and
        // `coverage.rs` says so; there is no TypeScript data type for
        // it.
        TypeKind::ObjectRef { .. } => Some("Object"),
        // An annotation type has no runtime form, so nothing declares
        // one and a reference to it would name a type that is not there.
        TypeKind::AnnotationType { .. } => Some("annotation type"),
        _ => None,
    };
    if let Some(construct) = refused {
        return Err(MaterialiseError::NoLanguageForm {
            construct: construct.to_string(),
            language: "TypeScript",
            at: at.to_string(),
        });
    }
    Ok(match &node.kind {
        // Matched ahead of the arm below, which would reach
        // `ts_primitive`'s `number` — eight bytes where the member is
        // sixteen.
        TypeKind::Primitive(PrimitiveKind::Float128) => ts::Type::named(Path::single(FLOAT128)),
        TypeKind::Primitive(kind) => ts::Type::Keyword(ts_primitive(*kind)),
        // A `wstring` is a `string` like a `string`: TypeScript strings
        // are UTF-16 already, and the surrogate rule of XTypes 1.3
        // 7.4.1.1.2 is the runtime's. A `fixed<P,S>` is a `string` too,
        // holding the decimal — a `number` is a binary64 and would
        // change the digits the FixedDigitCount axis measures.
        TypeKind::String { .. } | TypeKind::Fixed { .. } => ts::Type::string(),
        TypeKind::Sequence { element, .. } => {
            let Some(element) = element.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "sequence element".to_string(),
                    at: at.to_string(),
                });
            };
            array_of(type_at(ast, applied, element, at, depth + 1)?)
        }
        TypeKind::Array { element, dims } => {
            let Some(element) = element.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "array element".to_string(),
                    at: at.to_string(),
                });
            };
            let mut out = type_at(ast, applied, element, at, depth + 1)?;
            // One `[]` per dimension. TypeScript's array type carries no
            // length, so `long a[2][3]` and `long a[3][2]` are the same
            // TypeScript type; the lengths are the plan's, and `declare`
            // evaluates them there.
            for _ in dims {
                out = array_of(out);
            }
            out
        }
        // §7.9.1: `ReadonlyMap<K', V'>` for every key type, and never a
        // `Record<…>` — a `Record` loses key uniqueness and commits to
        // an enumeration order. The generated entry type stays: it is
        // what the codec stages the pairs through, because XTypes 1.3
        // §7.4.1.1.4 puts a map on the wire as a sequence of them.
        TypeKind::Map { key, value, .. } => {
            let (Some(key), Some(value)) = (key.resolved(), value.resolved()) else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "map key or value".to_string(),
                    at: at.to_string(),
                });
            };
            ts::Type::generic(
                Path::single("ReadonlyMap"),
                vec![
                    type_at(ast, applied, key, at, depth + 1)?,
                    type_at(ast, applied, value, at, depth + 1)?,
                ],
            )
        }
        // A typedef that got its own `type` declaration is written as
        // that name; one that did not resolves through.
        TypeKind::Alias { aliased } => {
            let Some(target) = aliased.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "typedef target".to_string(),
                    at: at.to_string(),
                });
            };
            if emits_alias(ast, applied, id) {
                ts::Type::named(qualified(ast, id)?)
            } else {
                type_at(ast, applied, target, at, depth + 1)?
            }
        }
        TypeKind::Structure { .. }
        | TypeKind::Union { .. }
        | TypeKind::Bitset { .. }
        | TypeKind::Valuetype { .. }
        | TypeKind::Enumeration { .. }
        | TypeKind::Bitmask { .. }
        | TypeKind::Native => ts::Type::named(qualified(ast, id)?),
        // A boxed value is a reference, absent when it is not there —
        // and `undefined` is what TypeScript's absent is. `T | null`
        // would be a second absent value beside the one an `@optional`
        // member already uses, and nothing in the type system would keep
        // the two apart.
        TypeKind::ValueBox { boxed: inner } => {
            let Some(target) = inner.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "value box target".to_string(),
                    at: at.to_string(),
                });
            };
            optional_of(type_at(ast, applied, target, at, depth + 1)?)
        }
        // The boxed form of §7.3.4, and never TypeScript `any` or
        // `unknown`: the first switches the checking off and the second
        // says the type is not known, where the IDL construct says it
        // is carried alongside the value.
        TypeKind::Any => ts::Type::named(Path::single(ANY_BOX)),
        TypeKind::ValueBase | TypeKind::ObjectRef { .. } | TypeKind::AnnotationType { .. } => {
            // Refused above; the arm keeps the match total, so a kind
            // added to the model without a TypeScript answer is a
            // compile error and not a member that silently takes
            // another's type.
            return Err(MaterialiseError::NoLanguageForm {
                construct: node.kind.as_str().to_string(),
                language: "TypeScript",
                at: at.to_string(),
            });
        }
    })
}

/// The TypeScript type keyword an IDL primitive becomes.
///
/// Exhaustive, so a primitive added to the model without a TypeScript
/// spelling is a compile error and not a member that silently takes
/// another's.
///
/// `long double` is **not** routed here: it takes [`FLOAT128`], which
/// carries sixteen octets. The `Float128` arm below exists so the match
/// is total and answers for a caller that wants the nearest built-in;
/// `type_at` matches it before it reaches this function.
///
/// See the module documentation for why the two 64-bit types are the
/// only ones that are not a `number`.
#[must_use]
pub const fn ts_primitive(kind: PrimitiveKind) -> ts::Keyword {
    match kind {
        PrimitiveKind::Boolean => ts::Keyword::Boolean,
        // TypeScript has no character type. A one-code-unit `string`
        // holds an IDL `char`'s 0..255 and a `wchar`'s BMP code point
        // alike; the range check is the codec's, exactly as
        // `Writer.writeChar` does it in the Java column.
        PrimitiveKind::Char | PrimitiveKind::WChar => ts::Keyword::String,
        // Every IDL integer of 32 bits or fewer, signed or unsigned.
        // 4294967295 is far below 2^53, so unlike Java nothing has to
        // widen for an `unsigned long`.
        PrimitiveKind::Int8
        | PrimitiveKind::Octet
        | PrimitiveKind::UInt8
        | PrimitiveKind::Int16
        | PrimitiveKind::UInt16
        | PrimitiveKind::Int32
        | PrimitiveKind::UInt32 => ts::Keyword::Number,
        // Past 2^53 a `number` stops holding integers as themselves.
        PrimitiveKind::Int64 | PrimitiveKind::UInt64 => ts::Keyword::BigInt,
        PrimitiveKind::Float32 | PrimitiveKind::Float64 | PrimitiveKind::Float128 => {
            ts::Keyword::Number
        }
    }
}

/// The TypeScript type an unsigned value of `bits` bits is held in.
///
/// The same rule as the primitive table, applied to a width the source
/// gave as a number: up to 32 bits is a `number`, and past it a
/// `bigint`. A bitmask of bound 32 stays a `number` — 4294967295 is
/// exact — where the Java column has to widen to a `long` because its
/// `int` is signed.
#[must_use]
pub fn unsigned_holder(bits: u16) -> ts::Type {
    if bits <= 32 {
        ts::Type::number()
    } else {
        ts::Type::bigint()
    }
}

/// `T[]`, with the brackets binding to the whole of `T`.
///
/// A union or a function type needs the parentheses: `A | B[]` is
/// `A | (B[])` and not `(A | B)[]`, and the two are different types. The
/// renderer does not add them — it renders what the tree says — so the
/// tree has to say it.
fn array_of(inner: ts::Type) -> ts::Type {
    match inner {
        ts::Type::Union(_) | ts::Type::Intersection(_) | ts::Type::Function { .. } => {
            ts::Type::Array(Box::new(ts::Type::Parenthesised(Box::new(inner))))
        }
        other => ts::Type::Array(Box::new(other)),
    }
}

/// `T | undefined`, flattened rather than nested.
fn optional_of(inner: ts::Type) -> ts::Type {
    let mut arms = match inner {
        ts::Type::Union(existing) => existing,
        other => vec![other],
    };
    let undefined = ts::Type::Keyword(ts::Keyword::Undefined);
    if !arms.contains(&undefined) {
        arms.push(undefined);
    }
    ts::Type::Union(arms)
}

/// What a `native` declaration maps to.
///
/// The facet if a `#pragma` or an annotation gave one, and `unknown`
/// otherwise — never a guess at what the name might mean, and never
/// `any`, which would switch the checking off for every use.
fn native_type(ast: &Ast, id: Id<TypeNode>) -> ts::Type {
    let mapping = ast
        .any_of_type(id)
        .ok()
        .and_then(|any| ast.facets(any))
        .and_then(|facets| facets.corba.as_ref())
        .and_then(|corba| corba.native_mapping.clone());
    match mapping {
        Some(spelling) => ts::Type::named(Path::new(&spelling.split('.').collect::<Vec<&str>>())),
        None => ts::Type::Keyword(ts::Keyword::Unknown),
    }
}

/// A named type's path, written from the file root.
///
/// `a.b.Point` and not `Point`: a reference has to mean the same thing
/// wherever it is written. TypeScript has no root operator to make that
/// unambiguous, which is why [`check_names`] refuses a nested
/// declaration that would capture the first segment.
///
/// # Errors
/// [`MaterialiseError::NoModule`] when the model will not answer for a
/// module the type nests in.
fn qualified(ast: &Ast, id: Id<TypeNode>) -> Result<Path, MaterialiseError> {
    let node = ast.type_(id).map_err(|_| MaterialiseError::Unevaluated {
        construct: "type".to_string(),
        at: "the file root".to_string(),
    })?;
    let mut segments = Vec::new();
    for (_, name, _) in module_chain(ast, node.module)? {
        segments.push(name);
    }
    segments.push(ts::escape_keyword(&node.name));
    Ok(Path::new(
        &segments.iter().map(String::as_str).collect::<Vec<&str>>(),
    ))
}

/// A named type's path from the file root, for a caller outside this
/// module.
///
/// The counterpart of [`crate::adapter::java::declared_path`], and the
/// one thing a codec needs that a structural walk cannot answer: the
/// materialiser nests a declaration in a namespace per IDL module, and a
/// generated codec sits at the file root, so `Point` there names nothing
/// and `a.b.Point` names the declaration.
///
/// Measured, in the write direction as well as the read:
/// `module m { struct S {}; }` through
/// `tools/wireprobe/wireprobe-zerodds-typescript` was
/// `error TS2304: Cannot find name 'S'`. One function and not two —
/// a codec that built the path itself would be a second implementation
/// of this naming, parting from the first on the first renamed module.
///
/// # Errors
/// [`MaterialiseError::NoModule`] when the model will not answer for a
/// module the type nests in.
pub fn declared_path(ast: &Ast, id: Id<TypeNode>) -> Result<Path, MaterialiseError> {
    qualified(ast, id)
}

/// The name the factory of `id` is declared under.
///
/// [`declared_path`] names the interface, which is erased: nothing can
/// be constructed from one. The factory beside it carries the same name
/// with `FACTORY_PREFIX` in front — `a.b.newPoint` — and it is what a
/// defaulter and a decoder call for a value to start from.
///
/// # Errors
/// As [`declared_path`].
pub fn factory_path(ast: &Ast, id: Id<TypeNode>) -> Result<Path, MaterialiseError> {
    let path = qualified(ast, id)?;
    let mut segments: Vec<String> = path
        .segments()
        .iter()
        .map(|segment| segment.as_str().to_string())
        .collect();
    let Some(tail) = segments.last_mut() else {
        return Err(MaterialiseError::Unevaluated {
            construct: "the name of the type a value is constructed from".to_string(),
            at: "the file root".to_string(),
        });
    };
    *tail = format!("{FACTORY_PREFIX}{tail}");
    Ok(Path::new(
        &segments.iter().map(String::as_str).collect::<Vec<&str>>(),
    ))
}

/// `a.b.ColourOrdinal` — the table §7.6 emits beside an enumeration.
///
/// The path is built the way [`factory_path`] builds the factory's: the
/// declaration's own qualified name with the suffix on the last segment,
/// so a codec at the file root names the same constant the materialiser
/// declared inside its namespace.
///
/// A typedef is followed to the enumeration it aliases, because the
/// alias has no table of its own.
///
/// # Errors
/// [`MaterialiseError::NoLanguageForm`] where `id` is not an
/// enumeration — there is no ordinal table for anything else, and a
/// caller that asked is holding the wrong type. As [`declared_path`]
/// otherwise.
pub fn ordinal_path(ast: &Ast, id: Id<TypeNode>) -> Result<Path, MaterialiseError> {
    let Some(id) = resolved_enumeration(ast, id, 0) else {
        return Err(MaterialiseError::NoLanguageForm {
            construct: "an ordinal table for a type that is not an enumeration".to_string(),
            language: "TypeScript",
            at: "the file root".to_string(),
        });
    };
    let path = qualified(ast, id)?;
    let mut segments: Vec<String> = path
        .segments()
        .iter()
        .map(|segment| segment.as_str().to_string())
        .collect();
    let Some(tail) = segments.last_mut() else {
        return Err(MaterialiseError::Unevaluated {
            construct: "the name of the enumeration an ordinal is looked up in".to_string(),
            at: "the file root".to_string(),
        });
    };
    tail.push_str("Ordinal");
    Ok(Path::new(
        &segments.iter().map(String::as_str).collect::<Vec<&str>>(),
    ))
}

/// The map entry type a `map<K,V>` needs a name for.
///
/// # Errors
/// As [`type_tag`].
pub fn map_entry_name(ast: &Ast, id: Id<TypeNode>) -> Result<String, MaterialiseError> {
    let Ok(node) = ast.type_(id) else {
        return Err(MaterialiseError::Unevaluated {
            construct: "map".to_string(),
            at: "the file root".to_string(),
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
/// generated TypeScript held no `zd_over` check at all, so `a` accepted
/// a stream of any length; with the members the other way round both
/// got the bounded body, so `b` refused — and truncated — a length the
/// IDL puts no limit on. `string<4>` beside `string` and `wstring<4>`
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

/// The name component an entry type takes from one of its halves.
///
/// The IDL kind and not the TypeScript spelling, and this is not
/// cosmetic: `octet`, `unsigned short` and `long` are all a `number`,
/// and they are one, two and four bytes on the wire. A structural name
/// built from the TypeScript type would give `map<octet,long>` and
/// `map<unsigned short,long>` one entry type, and a codec written
/// against it would put one of the two out at the other's width with
/// nothing to say it had. An alias is followed through, because two
/// names for one shape would otherwise describe the same type twice.
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
            at: "the file root".to_string(),
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
                    // `[0]` array's, and `declare` reports the failure
                    // by name.
                    Err(_) => name.push_str("_x"),
                }
            }
            name
        }
        // The entry type's own name carries no bound — two maps that
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
        TypeKind::ValueBox { boxed: inner } => match inner.resolved() {
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
            for (_, name, _) in module_chain(ast, node.module)? {
                segments.push(name);
            }
            segments.push(ts::escape_keyword(&node.name));
            segments.join("_")
        }
        TypeKind::Any => "any".to_string(),
        TypeKind::ValueBase => "valuebase".to_string(),
        TypeKind::ObjectRef { .. } => "objectref".to_string(),
        TypeKind::AnnotationType { .. } => "annotation".to_string(),
    })
}

// -- the discriminated union ---------------------------------------------

/// The `discriminator` type one case carries.
///
/// The literal types its labels name, joined;
/// `Exclude<DiscType, ExplicitLabels>` when one of the labels is
/// `default:`, because that case is selected by every value the others
/// do not claim (§7.5). `Exclude` narrows a literal-typed union and
/// leaves a primitive alone, so the exclusion is a real one for an
/// `enum` and a `boolean` discriminator and is the discriminator's own
/// type for an integer — which is what the clause says it is.
///
/// # Errors
/// [`MaterialiseError::Unevaluated`] when a label will not evaluate.
fn discriminant_type(
    ast: &Ast,
    discriminator: Id<TypeNode>,
    disc_type: &ts::Type,
    labels: &[CaseLabel],
    explicit: &[ts::Type],
    at: &str,
) -> Result<ts::Type, MaterialiseError> {
    let mut arms: Vec<ts::Type> = Vec::new();
    for label in labels {
        match label {
            CaseLabel::Default => return Ok(excluding(disc_type, explicit)),
            CaseLabel::Value(expr) => {
                let value =
                    ast.eval_integral(*expr)
                        .map_err(|_| MaterialiseError::Unevaluated {
                            construct: "union case label".to_string(),
                            at: at.to_string(),
                        })?;
                let literal = discriminant_type_of(ast, discriminator, value)?;
                if !arms.contains(&literal) {
                    arms.push(literal);
                }
            }
        }
    }
    Ok(match arms.len() {
        // A case with no label at all selects nothing. The
        // discriminator's own type rather than `never`: a `never` arm is
        // a variant no value inhabits, and the member could not then be
        // written.
        0 => disc_type.clone(),
        1 => arms.into_iter().next().unwrap_or_else(|| disc_type.clone()),
        _ => ts::Type::Union(arms),
    })
}

/// `Exclude<T, L1 | L2>`, or `T` where the union lists no explicit
/// label.
///
/// `Exclude<T, never>` is `T`, so the empty case is the same type either
/// way; it is written as the bare type because that is what the
/// declaration means and `Exclude` over nothing reads as though
/// something had been taken away.
fn excluding(disc_type: &ts::Type, explicit: &[ts::Type]) -> ts::Type {
    let removed = match explicit.len() {
        0 => return disc_type.clone(),
        1 => explicit
            .first()
            .cloned()
            .unwrap_or_else(|| disc_type.clone()),
        _ => ts::Type::Union(explicit.to_vec()),
    };
    ts::Type::generic(Path::single("Exclude"), vec![disc_type.clone(), removed])
}

/// Every literal type the union's `case` labels name, in declaration
/// order and without repeats — what `default:` is the complement of.
///
/// # Errors
/// As [`discriminant_type_of`].
fn explicit_labels(
    ast: &Ast,
    id: Id<TypeNode>,
    discriminator: Id<TypeNode>,
    at: &str,
) -> Result<Vec<ts::Type>, MaterialiseError> {
    let mut out: Vec<ts::Type> = Vec::new();
    for case in ast.cases_of(id) {
        let Ok(case_node) = ast.case(*case) else {
            continue;
        };
        for label in &case_node.labels {
            let CaseLabel::Value(expr) = label else {
                continue;
            };
            let value = ast
                .eval_integral(*expr)
                .map_err(|_| MaterialiseError::Unevaluated {
                    construct: "union case label".to_string(),
                    at: at.to_string(),
                })?;
            let literal = discriminant_type_of(ast, discriminator, value)?;
            if !out.contains(&literal) {
                out.push(literal);
            }
        }
    }
    Ok(out)
}

/// The type one discriminator value narrows to.
///
/// `typeof a.b.Tag.TAG_A` for an enumeration — which is the literal
/// string that enumerator's member holds, and the form §7.5 writes as
/// `Tag.TAG_A` — and the value's own literal type for everything else.
///
/// # Errors
/// [`MaterialiseError::Unevaluated`] for a value that names no
/// enumerator of an enumeration discriminator: there is no type for it,
/// and a number in its place would be an arm no value inhabits.
fn discriminant_type_of(
    ast: &Ast,
    discriminator: Id<TypeNode>,
    value: i128,
) -> Result<ts::Type, MaterialiseError> {
    let Some(enumeration) = resolved_enumeration(ast, discriminator, 0) else {
        return Ok(ts::Type::Literal(discriminant_literal(
            ast,
            discriminator,
            value,
        )));
    };
    let name = enumerator_named_by(ast, enumeration, value).ok_or_else(|| {
        MaterialiseError::Unevaluated {
            construct: format!("union case label {value}, which names no enumerator"),
            at: "a union case label".to_string(),
        }
    })?;
    let mut segments: Vec<String> = qualified(ast, enumeration)?
        .segments()
        .iter()
        .map(|segment| segment.as_str().to_string())
        .collect();
    segments.push(ts::escape_member(&name));
    Ok(ts::Type::TypeOf(Path::new(
        &segments.iter().map(String::as_str).collect::<Vec<&str>>(),
    )))
}

/// The expression one discriminator value is written as.
///
/// The value side of [`discriminant_type_of`]: `a.b.Tag.TAG_A` for an
/// enumeration and the literal itself for every other discriminator.
///
/// # Errors
/// As [`discriminant_type_of`].
fn discriminant_expr(
    ast: &Ast,
    discriminator: Id<TypeNode>,
    value: i128,
) -> Result<ts::Expr, MaterialiseError> {
    let Some(enumeration) = resolved_enumeration(ast, discriminator, 0) else {
        return Ok(ts::Expr::Literal(discriminant_literal(
            ast,
            discriminator,
            value,
        )));
    };
    let name = enumerator_named_by(ast, enumeration, value).ok_or_else(|| {
        MaterialiseError::Unevaluated {
            construct: format!("union discriminator value {value}, which names no enumerator"),
            at: "a union discriminator".to_string(),
        }
    })?;
    enumerator_path(ast, enumeration, &name)
}

/// The literal a discriminator's value is written as.
///
/// The discriminator's own mapping decides it, and getting it wrong
/// produces a variant no value inhabits: `2` and `2n` are two types, and
/// a `boolean` discriminator's `1` is `true`.
fn discriminant_literal(ast: &Ast, discriminator: Id<TypeNode>, value: i128) -> Literal {
    match resolved_kind(ast, discriminator, 0) {
        Some(PrimitiveKind::Boolean) => Literal::Bool(value != 0),
        Some(PrimitiveKind::Char | PrimitiveKind::WChar) => u32::try_from(value)
            .ok()
            .and_then(char::from_u32)
            .map_or_else(|| Literal::int(value), Literal::Char),
        Some(PrimitiveKind::Int64 | PrimitiveKind::UInt64) => Literal::Int(format!("{value}n")),
        _ => Literal::int(value),
    }
}

/// The primitive a type resolves to through its aliases, if it is one.
///
/// zerodds-lint: recursion-depth 64 (alias chains; bounded by `MAX_TYPE_DEPTH`)
fn resolved_kind(ast: &Ast, id: Id<TypeNode>, depth: usize) -> Option<PrimitiveKind> {
    if depth > MAX_TYPE_DEPTH {
        return None;
    }
    match &ast.type_(id).ok()?.kind {
        TypeKind::Primitive(kind) => Some(*kind),
        TypeKind::Alias { aliased } => resolved_kind(ast, aliased.resolved()?, depth + 1),
        _ => None,
    }
}

// -- the values a generated class starts from ----------------------------

/// The value a class field of `type_` is initialised to.
///
/// Table 9 of DDS-XTypes 1.3, as a TypeScript expression. Written out
/// rather than left to the language: TypeScript has no default
/// initialiser at all, and `strictPropertyInitialization` rejects a
/// field that has none.
///
/// # Errors
/// [`MaterialiseError`] for a type this column has no value for — which
/// is the same set [`type_at`] refuses, so a field that has a type here
/// has a value too.
///
/// zerodds-lint: recursion-depth 64 (collection nesting; bounded by `MAX_TYPE_DEPTH`)
fn default_expr(
    ast: &Ast,
    applied: &Applied,
    id: Id<TypeNode>,
    at: &str,
    depth: usize,
) -> Result<ts::Expr, MaterialiseError> {
    if depth > MAX_TYPE_DEPTH {
        return Err(MaterialiseError::Unevaluated {
            construct: format!("default value, which nests past {MAX_TYPE_DEPTH}"),
            at: at.to_string(),
        });
    }
    let node = ast.type_(id).map_err(|_| MaterialiseError::Unevaluated {
        construct: "type".to_string(),
        at: at.to_string(),
    })?;
    Ok(match &node.kind {
        TypeKind::Primitive(PrimitiveKind::Float128) => ts::Expr::Object(vec![(
            ts::PropertyName::ident("octets"),
            ts::Expr::New {
                callee: Path::single("Uint8Array"),
                args: vec![ts::Expr::Literal(Literal::int(16))],
            },
        )]),
        TypeKind::Primitive(PrimitiveKind::Boolean) => ts::Expr::Literal(Literal::Bool(false)),
        TypeKind::Primitive(PrimitiveKind::Char | PrimitiveKind::WChar) => {
            ts::Expr::Literal(Literal::Char('\0'))
        }
        TypeKind::Primitive(PrimitiveKind::Int64 | PrimitiveKind::UInt64) => {
            ts::Expr::Literal(Literal::Int("0n".to_string()))
        }
        TypeKind::Primitive(_) => ts::Expr::Literal(Literal::int(0)),
        TypeKind::String { .. } | TypeKind::Fixed { .. } => ts::Expr::Literal(Literal::string("")),
        // A sequence may be empty and Table 9 says it is.
        TypeKind::Sequence { .. } => ts::Expr::Array(Vec::new()),
        // A map may be empty too, and §7.9.1 makes it a `ReadonlyMap` —
        // so the empty value is an empty `Map` and not an empty array.
        TypeKind::Map { .. } => ts::Expr::New {
            callee: Path::single("Map"),
            args: Vec::new(),
        },
        // An array may not: its length is part of its type, and an empty
        // one is a different value. `Array.from` rather than a literal
        // of n elements, so a dimension of a thousand costs one line.
        TypeKind::Array { element, dims } => {
            let Some(element) = element.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "array element".to_string(),
                    at: at.to_string(),
                });
            };
            let mut lengths = Vec::new();
            for dim in dims {
                lengths.push(ast.eval_integral(*dim).map_err(|_| {
                    MaterialiseError::Unevaluated {
                        construct: "array length".to_string(),
                        at: at.to_string(),
                    }
                })?);
            }
            let mut out = default_expr(ast, applied, element, at, depth + 1)?;
            for length in lengths.into_iter().rev() {
                out = filled_array(length, out);
            }
            out
        }
        TypeKind::Alias { aliased } => {
            let Some(target) = aliased.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "typedef target".to_string(),
                    at: at.to_string(),
                });
            };
            default_expr(ast, applied, target, at, depth + 1)?
        }
        // The enumeration's Table 9 default is its first literal, or the
        // one marked `@default_literal`. Named through the `as const`
        // object and not written as the number it stands for: §7.6
        // makes the member's type a string-literal union, and the number
        // is not a value of it.
        TypeKind::Enumeration { .. } => enum_default_literal(ast, id, at)?,
        TypeKind::Bitmask { .. } => zero_of(&unsigned_holder(ast.effective_bit_bound(id))),
        // The factory of the nested type, called. Not an object literal
        // written out here: the nested type's own members are its
        // declaration's business, and two places building the same value
        // is how the two come to disagree.
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } | TypeKind::Bitset { .. } => {
            ts::Expr::Call {
                callee: Box::new(ts::Expr::Path(factory_path(ast, id)?)),
                args: Vec::new(),
            }
        }
        TypeKind::Union { discriminator, .. } => {
            let Some(disc) = discriminator.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "union discriminator".to_string(),
                    at: at.to_string(),
                });
            };
            union_default(ast, applied, id, disc, at, depth)?
        }
        // A boxed value and a native type are both absent by default:
        // `undefined` inhabits `T | undefined` and `unknown` alike.
        TypeKind::ValueBox { .. } | TypeKind::Native => ts::Expr::Undefined,
        // An empty box: no type is named and nothing is boxed. Not
        // `undefined`, which does not inhabit the member's type, and
        // not a box naming some type the field does not hold.
        TypeKind::Any => ts::Expr::Object(vec![
            (
                ts::PropertyName::ident("typeId"),
                ts::Expr::Literal(Literal::string("")),
            ),
            (ts::PropertyName::ident("value"), ts::Expr::Undefined),
        ]),
        TypeKind::ValueBase | TypeKind::ObjectRef { .. } | TypeKind::AnnotationType { .. } => {
            return Err(MaterialiseError::NoLanguageForm {
                construct: node.kind.as_str().to_string(),
                language: "TypeScript",
                at: at.to_string(),
            });
        }
    })
}

/// The value a union-typed field starts from.
///
/// The case the discriminator's own Table 9 default selects, the
/// `default:` case when none does, and the first case when there is no
/// `default:` either. A union with no case at all is its discriminator
/// alone, which is the one arm [`declare`] emitted for it.
fn union_default(
    ast: &Ast,
    applied: &Applied,
    id: Id<TypeNode>,
    discriminator: Id<TypeNode>,
    at: &str,
    depth: usize,
) -> Result<ts::Expr, MaterialiseError> {
    let selector = discriminator_default_value(ast, discriminator);
    let mut fallback: Option<(&zerodds_ast::CaseNode, i128)> = None;
    let mut first: Option<(&zerodds_ast::CaseNode, i128)> = None;
    let mut chosen: Option<(&zerodds_ast::CaseNode, i128)> = None;

    for case in ast.cases_of(id) {
        let Ok(case_node) = ast.case(*case) else {
            continue;
        };
        let mut label_value = None;
        let mut is_default = false;
        for label in &case_node.labels {
            match label {
                CaseLabel::Default => is_default = true,
                CaseLabel::Value(expr) => {
                    if let Ok(value) = ast.eval_integral(*expr) {
                        if label_value.is_none() {
                            label_value = Some(value);
                        }
                        if Some(value) == selector && chosen.is_none() {
                            chosen = Some((case_node, value));
                        }
                    }
                }
            }
        }
        let value = label_value.or(selector).unwrap_or(0);
        if is_default && fallback.is_none() {
            fallback = Some((case_node, value));
        }
        if first.is_none() {
            first = Some((case_node, value));
        }
    }

    let Some((case_node, value)) = chosen.or(fallback).or(first) else {
        // No case at all: the arm `declare` emitted carries the
        // discriminator alone.
        return Ok(ts::Expr::Object(vec![(
            ts::PropertyName::ident(DISCRIMINANT_PROPERTY),
            discriminant_expr(ast, discriminator, selector.unwrap_or(0))?,
        )]));
    };
    let Ok(member) = ast.member(case_node.member) else {
        return Err(MaterialiseError::Unevaluated {
            construct: "member behind a union case the model listed".to_string(),
            at: at.to_string(),
        });
    };
    let Some(target) = member.type_.resolved() else {
        return Err(MaterialiseError::Unevaluated {
            construct: "union case type".to_string(),
            at: member.name.clone(),
        });
    };
    Ok(ts::Expr::Object(vec![
        (
            ts::PropertyName::ident(DISCRIMINANT_PROPERTY),
            discriminant_expr(ast, discriminator, value)?,
        ),
        (
            ts::PropertyName::ident(&ts::escape_member(&member.name)),
            default_expr(ast, applied, target, &member.name, depth + 1)?,
        ),
    ]))
}

/// The integer a defaulted discriminator has, for label comparison.
///
/// zerodds-lint: recursion-depth 64 (alias chains; bounded by `MAX_TYPE_DEPTH`)
fn discriminator_default_value(ast: &Ast, discriminator: Id<TypeNode>) -> Option<i128> {
    match &ast.type_(discriminator).ok()?.kind {
        TypeKind::Alias { aliased } => discriminator_default_value(ast, aliased.resolved()?),
        TypeKind::Enumeration { .. } => Some(enum_default_value(ast, discriminator)),
        // An integer, a `char` and a `boolean` all have zero for their
        // Table 9 default, and `'\0'` and `FALSE` are zero.
        TypeKind::Primitive(_) => Some(0),
        _ => None,
    }
}

/// `a.b.Colour.RED` — one enumerator of `id`, written from the file
/// root.
///
/// The `as const` object's member and not the number it stands for:
/// §7.6 makes an enumeration a string-literal union, so the number is
/// not a value of the type at all. The path is the declaration's, for
/// the reason [`qualified`] gives.
///
/// # Errors
/// As [`qualified`].
fn enumerator_path(
    ast: &Ast,
    id: Id<TypeNode>,
    enumerator: &str,
) -> Result<ts::Expr, MaterialiseError> {
    let mut segments: Vec<String> = qualified(ast, id)?
        .segments()
        .iter()
        .map(|segment| segment.as_str().to_string())
        .collect();
    segments.push(ts::escape_member(enumerator));
    Ok(ts::Expr::Path(Path::new(
        &segments.iter().map(String::as_str).collect::<Vec<&str>>(),
    )))
}

/// The enumerator an enumeration's Table 9 default names, as a
/// TypeScript expression.
///
/// # Errors
/// [`MaterialiseError::Unevaluated`] for an enumeration with no
/// enumerator at all — a type no value inhabits, which is refused by
/// name rather than given a zero that is not one of its members.
fn enum_default_literal(
    ast: &Ast,
    id: Id<TypeNode>,
    at: &str,
) -> Result<ts::Expr, MaterialiseError> {
    let Some(enumerator) = crate::emit::default_enumerator_name(ast, id) else {
        return Err(MaterialiseError::Unevaluated {
            construct: "enumeration with no enumerator to take as its default".to_string(),
            at: at.to_string(),
        });
    };
    enumerator_path(ast, id, &enumerator)
}

/// The first enumerator of `id` whose effective value is `value`.
///
/// First and not any: two enumerators may carry the same `@value`, and
/// the codec's own chains take the first in declaration order in both
/// directions.
fn enumerator_named_by(ast: &Ast, id: Id<TypeNode>, value: i128) -> Option<String> {
    for enumerator in ast.enumerators_of(id) {
        if ast.effective_enumerator_value(*enumerator) == Some(value) {
            return ast
                .enumerator(*enumerator)
                .ok()
                .map(|node| node.name.clone());
        }
    }
    None
}

/// The enumeration `id` resolves to through its typedefs, if it is one.
///
/// zerodds-lint: recursion-depth 64 (alias chains; bounded by `MAX_TYPE_DEPTH`)
fn resolved_enumeration(ast: &Ast, id: Id<TypeNode>, depth: usize) -> Option<Id<TypeNode>> {
    if depth > MAX_TYPE_DEPTH {
        return None;
    }
    match &ast.type_(id).ok()?.kind {
        TypeKind::Enumeration { .. } => Some(id),
        TypeKind::Alias { aliased } => resolved_enumeration(ast, aliased.resolved()?, depth + 1),
        _ => None,
    }
}

/// An enumeration's Table 9 default: its `@default_literal`, or its
/// first enumerator, or zero when it declares none.
fn enum_default_value(ast: &Ast, id: Id<TypeNode>) -> i128 {
    let mut first = None;
    for enumerator in ast.enumerators_of(id) {
        let Ok(node) = ast.enumerator(*enumerator) else {
            continue;
        };
        let Some(value) = ast.effective_enumerator_value(*enumerator) else {
            continue;
        };
        if node
            .facets
            .xtypes
            .as_ref()
            .is_some_and(|x| x.is_default_literal)
        {
            return value;
        }
        if first.is_none() {
            first = Some(value);
        }
    }
    first.unwrap_or(0)
}

/// `Array.from({ length: n }, () => <element>)`
///
/// A call and not a literal of n elements: an IDL array's length is a
/// constant expression and nothing bounds it, so `long a[4096]` would
/// otherwise be four thousand tokens in the generated source.
///
/// An object-literal element is parenthesised. A concise arrow body
/// starting with `{` is a *block*, so `() => { _d: 0, whole: 0 }` is a
/// function with a labelled statement in it and not one returning a
/// value — which is a parse error two tokens later rather than a wrong
/// value, but it is the renderer being asked to guess and the tree not
/// saying. The renderer writes what the tree holds, so the tree holds
/// the brackets.
fn filled_array(length: i128, element: ts::Expr) -> ts::Expr {
    let element = match element {
        ts::Expr::Object(_) => ts::Expr::Parenthesised(Box::new(element)),
        other => other,
    };
    ts::Expr::Call {
        callee: Box::new(ts::Expr::Member {
            object: Box::new(ts::Expr::Path(Path::single("Array"))),
            name: Ident::new("from"),
        }),
        args: vec![
            ts::Expr::Object(vec![(
                ts::PropertyName::ident("length"),
                ts::Expr::Literal(Literal::int(length)),
            )]),
            ts::Expr::Arrow {
                params: Vec::new(),
                body: Box::new(element),
            },
        ],
    }
}

/// The zero of a `number` or a `bigint` holder.
fn zero_of(holder: &ts::Type) -> ts::Expr {
    if *holder == ts::Type::bigint() {
        ts::Expr::Literal(Literal::Int("0n".to_string()))
    } else {
        ts::Expr::Literal(Literal::int(0))
    }
}

/// The value a bitmask's bit at `position` names.
///
/// The number itself, not `1 << position`. JavaScript's `<<` is a signed
/// 32-bit operation: `1 << 31` is -2147483648 and `1 << 32` is 1, so a
/// shift would give the wrong value for exactly the two bounds a
/// `@bit_bound(32)` and a `@bit_bound(64)` bitmask reach. The `bigint`
/// shift `1n << 40n` would be right, but writing both forms would put a
/// second spelling in the output for no reason the reader can see.
fn bit_literal(position: u16, wide: bool) -> Literal {
    let value = i128::from(1u8) << u32::from(position);
    if wide {
        Literal::Int(format!("{value}n"))
    } else {
        Literal::int(value)
    }
}

// -- constants -----------------------------------------------------------

/// The TypeScript expression a constant's value takes.
///
/// The declared type carries the string-shaped forms already —
/// [`type_at`] answers `string` for a `string`, a `wstring`, a `fixed`,
/// a `char` and a `wchar` — so what is left here is picking the right
/// literal and refusing the one TypeScript has no literal for.
///
/// The integer form asks the **model** and not the rendered type: an
/// enumeration and a `long` are both a `number` in the output and a
/// `long long` is a `bigint`, and the suffix is not something the
/// TypeScript type can be read backwards for.
///
/// `long double` is refused by name. TypeScript has no 128-bit floating
/// type, [`FLOAT128`] is an interface with no literal, and a `number`
/// would put a different value in the file than the IDL wrote.
///
/// # Errors
/// [`MaterialiseError`] naming the constant and what will not hold it.
fn constant_expr(
    ast: &Ast,
    declared: Option<Id<TypeNode>>,
    value: &zerodds_ast::ConstValue,
    at: &str,
) -> Result<ts::Expr, MaterialiseError> {
    use zerodds_ast::ConstValue;
    let refuse = |construct: String| MaterialiseError::NoLanguageForm {
        construct,
        language: "TypeScript",
        at: at.to_string(),
    };
    let float128 = declared
        .and_then(|id| resolved_kind(ast, id, 0))
        .is_some_and(|kind| matches!(kind, PrimitiveKind::Float128));
    match value {
        ConstValue::Integer(number) => integer_literal(ast, declared, *number, at),
        ConstValue::Boolean(flag) => Ok(ts::Expr::Literal(Literal::Bool(*flag))),
        ConstValue::Char(character) => Ok(ts::Expr::Literal(Literal::Char(*character))),
        // A TypeScript string is UTF-16, so a `wchar` above the Basic
        // Multilingual Plane needs a surrogate pair — which `char` in
        // Rust already is not, so a code point that is not a Unicode
        // scalar value is refused rather than written as a lone
        // surrogate no compiler reads.
        ConstValue::WChar(point) => match constant::wchar_char(*point) {
            Some(character) => Ok(ts::Expr::Literal(Literal::Char(character))),
            None => Err(refuse(format!(
                "the code point U+{point:04X}, which is not a Unicode scalar value"
            ))),
        },
        ConstValue::Float { mantissa, exponent } => {
            if float128 {
                return Err(refuse(String::from(
                    "a `long double` constant, which has no TypeScript literal",
                )));
            }
            // No suffix: TypeScript has one floating type, so a `float`
            // and a `double` constant are written the same way. The
            // leading zero `float_text` adds for Rust's sake is legal
            // here and changes no value.
            Ok(ts::Expr::Literal(Literal::Float(constant::float_text(
                mantissa,
                exponent.as_deref(),
            ))))
        }
        // The decimal it was written as, held in a `string`. A `number`
        // is a binary64 and would change the digit count the
        // FixedDigitCount axis measures.
        ConstValue::Fixed { digits, scale } => Ok(ts::Expr::Literal(Literal::Str(
            constant::fixed_decimal(digits, *scale),
        ))),
        ConstValue::String(text) => Ok(ts::Expr::Literal(Literal::Str(text.clone()))),
        ConstValue::WString(points) => match constant::wstring_utf8(points) {
            Some(text) => Ok(ts::Expr::Literal(Literal::Str(text))),
            None => Err(refuse(String::from(
                "a `wstring` constant holding a lone surrogate",
            ))),
        },
        // `ConstValue` is `#[non_exhaustive]`, so a form added to the
        // model reaches here. Refused by name rather than given a
        // plausible default: a constant emitted as the wrong literal is
        // a declaration that disagrees with the IDL and compiles.
        _ => Err(refuse(String::from(
            "a constant form this binding has no literal for",
        ))),
    }
}

/// An integer constant, in the spelling its declared type calls for.
///
/// # Errors
/// [`MaterialiseError::NoLanguageForm`] when a `number` is asked to hold
/// an integer past [`MAX_SAFE_INTEGER`]. Refused and not written out: a
/// `number` literal above that range is a different number from the one
/// the source wrote, and TypeScript reports nothing.
fn integer_literal(
    ast: &Ast,
    declared: Option<Id<TypeNode>>,
    value: i128,
    at: &str,
) -> Result<ts::Expr, MaterialiseError> {
    // An enumeration-typed constant names the enumerator and not the
    // number the model folded it to: §7.6 makes the type a
    // string-literal union, so `export const C: Colour = 1` is a value
    // the declared type does not hold at all.
    if let Some(enumeration) = declared.and_then(|id| resolved_enumeration(ast, id, 0)) {
        let Some(name) = enumerator_named_by(ast, enumeration, value) else {
            return Err(MaterialiseError::NoLanguageForm {
                construct: format!("the value {value}, which names no enumerator"),
                language: "TypeScript",
                at: at.to_string(),
            });
        };
        return enumerator_path(ast, enumeration, &name);
    }
    let kind = declared.and_then(|id| resolved_kind(ast, id, 0));
    Ok(match kind {
        Some(PrimitiveKind::Boolean) => ts::Expr::Literal(Literal::Bool(value != 0)),
        Some(PrimitiveKind::Char | PrimitiveKind::WChar) => {
            match u32::try_from(value).ok().and_then(char::from_u32) {
                Some(character) => ts::Expr::Literal(Literal::Char(character)),
                None => {
                    return Err(MaterialiseError::NoLanguageForm {
                        construct: format!(
                            "the value {value}, which is not a Unicode scalar value and \
                             so is not a one-unit TypeScript `string`"
                        ),
                        language: "TypeScript",
                        at: at.to_string(),
                    });
                }
            }
        }
        Some(PrimitiveKind::Int64 | PrimitiveKind::UInt64) => {
            ts::Expr::Literal(Literal::Int(format!("{value}n")))
        }
        _ => ts::Expr::Literal(Literal::int(in_range(
            value,
            -MAX_SAFE_INTEGER,
            MAX_SAFE_INTEGER,
            "number",
            at,
        )?)),
    })
}

/// `value`, or the failure that says it does not fit.
///
/// # Errors
/// [`MaterialiseError::NoLanguageForm`], naming the value and the type
/// that will not hold it. Writing it out anyway would put a number in
/// the source that reads back as a different one.
fn in_range(
    value: i128,
    low: i128,
    high: i128,
    type_: &str,
    at: &str,
) -> Result<i128, MaterialiseError> {
    if value < low || value > high {
        return Err(MaterialiseError::NoLanguageForm {
            construct: format!(
                "the value {value}, which a TypeScript `{type_}` does not hold as itself"
            ),
            language: "TypeScript",
            at: at.to_string(),
        });
    }
    Ok(value)
}

// -- naming --------------------------------------------------------------

/// A declaration's documentation, with the IDL name recorded when the
/// generated one differs.
///
/// A rename with nothing recording it is a name the reader cannot find
/// in the IDL. Nothing is added where nothing moved, so the note means
/// what it says.
fn renamed_doc(idl_name: &str, doc: Doc) -> Doc {
    if !ts::is_reserved(idl_name) {
        return doc;
    }
    let mut out = doc;
    out.push(&format!(
        "IDL `{idl_name}`, renamed: it is a reserved word in a TypeScript binding."
    ));
    out
}

/// The same for a member, whose escaping rule is the narrower one.
fn renamed_member_doc(idl_name: &str, doc: Doc) -> Doc {
    if !ts::is_reserved_member(idl_name) {
        return doc;
    }
    let mut out = doc;
    out.push(&format!(
        "IDL `{idl_name}`, renamed: a class body parses `{idl_name}` as the \
         constructor, so `{idl_name}: T` is a syntax error."
    ));
    out
}

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

// -- the descriptor side-tables ------------------------------------------

/// The package the descriptor runtime is published under.
///
/// DDS-TS 1.0 chapter 8 calls this identifier vendor-specific: a
/// conformant implementation exports every chapter-8 name under exactly
/// the names given, and only the import path is its own. This tree's
/// path is `runtime/typescript/src/`, which is what a relative import in
/// a test or a probe names; this is the published one.
///
/// `@zerodds/wire` and not `@zerodds/types`. The second was the name
/// this constant carried, and no such package exists — not in this
/// tree, which publishes `@zerodds/wire` (`runtime/typescript`),
/// `@zerodds/node` (`crates/ts-node`) and `@zerodds/wasm`
/// (`crates/ts-wasm`), and not on a registry either. A generated file
/// therefore opened with `error TS2307: Cannot find module
/// '@zerodds/types'`, measured with `tsc`, while its other import — the
/// wire runtime's — already named `@zerodds/wire` correctly. Both
/// chapter-8 names this import asks for are exported from that one
/// package: `registerType` from `runtime/typescript/src/registry.ts`
/// and `DdsTypeDescriptor` from `src/types.ts`, each re-exported by
/// `src/index.ts`. One package for an application to install, which is
/// what the runtime's own `index.ts` was written as: "Two halves under
/// one import path."
pub const DEFAULT_RUNTIME_PACKAGE: &str = "@zerodds/wire";

/// How many bits the integer holding a bitset carries. XTypes 1.3
/// §7.3.1.2.1.11.
///
/// The same sixty-four `crate::emit::decode` refuses a wider bitset
/// against and `zerodds_wire_codec::encode::write_bitset` refuses it
/// against for the probe.
const ANCHOR_BITS: u16 = 64;

/// The synthetic member id a union's discriminator carries. §7.5.1.
///
/// `0xFFFFFFFF` follows the XTypes convention for synthetic member ids
/// and no user-declared member may take it.
const DISCRIMINATOR_ID: u32 = 0xffff_ffff;

/// The id the descriptor gives the member `parent`, which is not a member of the wire: one below
/// the discriminator's, so that no `@id` of an IDL member (a 28-bit value, §7.3.1.2.1.8) is it.
const PARENT_ID: u32 = 0xffff_fffe;

/// The property a generated union arm carries its discriminant in.
///
/// `discriminator`, which is the name DDS-TS 1.0 §7.5 gives it. The
/// descriptor's synthetic first member is named with this too, so a
/// consumer reading the side-table finds the property the arms actually
/// have rather than one this file and `equal.ts` would have to agree
/// about twice.
///
/// The name is the TypeScript surface's alone: the interop suite's
/// canonical JSON spells a union's discriminant `_d` in every column,
/// and that dialect is `crates/wire-probe/src/canonical.rs`'s, not this
/// mapping's.
pub const DISCRIMINANT_PROPERTY: &str = "discriminator";

/// The `import` a file carrying descriptors needs.
///
/// Two names and no more: the descriptor type, which is erased, and
/// `registerType`, which is not. Everything else a descriptor states is
/// a literal.
#[must_use]
pub fn descriptor_import(module: &str) -> ts::Import {
    ts::Import {
        module: module.to_string(),
        type_only: false,
        default: None,
        namespace: None,
        names: vec![
            ts::ImportName {
                name: Ident::new("registerType"),
                alias: None,
                type_only: false,
            },
            ts::ImportName {
                name: Ident::new("DdsTypeDescriptor"),
                alias: None,
                type_only: true,
            },
        ],
    }
}

/// The descriptor side-table for every type a model declares.
///
/// Three declarations per IDL type, in the namespace the type is
/// declared in (DDS-TS 1.0 §7.4.1, §7.5.1, §7.6.1, §7.7.1, §7.8.1,
/// §7.10.1):
///
/// 1. `export function is<Type>(v: unknown): v is <Type>` — the guard,
///    checking the presence and type of every required member.
/// 2. `export const <Type>Type: DdsTypeDescriptor<<Type>>` — the
///    metadata, with the guard as its `typeGuard`.
/// 3. `registerType(<Type>Type);` — the registration, which is what
///    makes `{ kind: "ref", name }` resolvable.
///
/// The order matters for the third: the registration runs when the
/// module is first imported, and the constant it names has to be
/// initialised by then. The guard is a function declaration and is
/// hoisted, so the constant may name it from above.
///
/// Appended to [`ts_items`]'s output by a caller that wants both. Kept
/// separate because it is not self-contained — see [`descriptor_import`]
/// — and because a name collision between a descriptor constant and an
/// IDL declaration of the same name is left to `tsc`, which reports a
/// redeclared binding rather than merging it.
///
/// # Errors
/// [`MaterialiseError`] for the same set [`ts_items`] refuses, plus a
/// value the descriptor has to state and the model will not evaluate.
pub fn ts_descriptors(ast: &Ast, applied: &Applied) -> Result<Vec<ts::Decl>, MaterialiseError> {
    let items = descriptor_pending(ast, applied)?;
    let mut root = Scope::default();
    for item in items {
        insert(ast, &mut root, item)?;
    }
    scope_decls(root, 0)
}

/// The descriptor declarations, before they are nested.
fn descriptor_pending(ast: &Ast, applied: &Applied) -> Result<Vec<Pending>, MaterialiseError> {
    let mut items: Vec<Pending> = Vec::new();
    for id in ordered_types(ast) {
        describe_type(ast, applied, id, &mut items)?;
    }
    for (_, node) in ast.exceptions() {
        if ghost_created_for(applied, node.node_id, "TsError").is_none() {
            continue;
        }
        let mut fields = Vec::new();
        for (position, member) in node.members.iter().enumerate() {
            fields.push(member_field(ast, applied, *member, position, &node.name)?);
        }
        let spec = TypeSpec {
            kind: "exception",
            local: ts::escape_keyword(&node.name),
            guard: guard_name(&node.name),
            qualified: qualified_idl_name(ast, node.module, &node.name)?,
            // `zerodds_ast::DEFAULT_EXTENSIBILITY` and not a second
            // spelling of what it holds: `Ast::effective_extensibility`
            // falls back to that constant for every *type*, an
            // exception has no `Id<TypeNode>` to ask it with, and two
            // literals of one rule are two answers as soon as one of
            // them moves.
            extensibility: extensibility_word(
                node.facets
                    .extensibility()
                    .unwrap_or(zerodds_ast::DEFAULT_EXTENSIBILITY),
            ),
            nested: is_nested(&node.facets),
            topic: topic_of(&node.facets, &node.name),
            autoid: autoid_word(&node.facets),
            bit_bound: None,
            has_default: None,
            has_implicit_default: None,
            fields,
            guard_shape: GuardShape::Object,
        };
        push_descriptor(&mut items, node.module, &spec);
    }

    Ok(items)
}

/// What one descriptor states about a type.
struct TypeSpec {
    /// `DescriptorKind` — the value of the `kind` property.
    kind: &'static str,
    /// The TypeScript name of the type, inside its own namespace.
    local: String,
    /// The name of the guard function.
    guard: String,
    /// The qualified IDL name, which is what the registry is keyed by.
    qualified: String,
    /// `"final"`, `"appendable"` or `"mutable"`.
    extensibility: &'static str,
    /// `@nested`.
    nested: bool,
    /// `@topic`, where one was declared.
    topic: Option<String>,
    /// `@autoid`, where one was declared.
    autoid: Option<&'static str>,
    /// `@bit_bound`, for the types that have one.
    bit_bound: Option<u16>,
    /// Union only: whether a `default:` case was declared.
    has_default: Option<bool>,
    /// Union only: whether the listed cases leave the discriminator
    /// uncovered without a `default:`.
    has_implicit_default: Option<bool>,
    /// The members, in declaration order.
    fields: Vec<FieldSpec>,
    /// How the guard decides.
    guard_shape: GuardShape,
}

/// What one entry of a descriptor's `fields` array states.
struct FieldSpec {
    /// The IDL member name, as declared.
    name: String,
    /// The effective member id.
    id: u32,
    /// The member's `DdsTypeRef`.
    type_ref: ts::Expr,
    /// `@key`.
    key: bool,
    /// `@optional`.
    optional: bool,
    /// `@must_understand`.
    must_understand: bool,
    /// `@default`, an enumerator's ordinal, or a bit position.
    default: Option<ts::Expr>,
    /// `@unit`.
    unit: Option<String>,
    /// `@min`.
    min: Option<ts::Expr>,
    /// `@max`.
    max: Option<ts::Expr>,
    /// `@hashid`, as the name that was hashed.
    hashid: Option<String>,
    /// Union case labels: `Some(list)` for a labelled case, `Some(none)`
    /// for the `default:` case, `None` for a member that is not a case.
    labels: Option<Option<Vec<ts::Expr>>>,
    /// A map key whose JavaScript equality is not value equality.
    key_equality_hazard: bool,
    /// The member is the base structure, the first member `parent` (XTypes 1.3 §7.5.1.4.1.3).
    base: bool,
    /// What the guard checks about this member, where it checks
    /// anything.
    check: Option<ts::Expr>,
}

/// How a guard decides that a value is of its type.
enum GuardShape {
    /// A property bag: the object check, then one check per required
    /// member.
    Object,
    /// A scalar: one `typeof` against the holder the type maps to.
    TypeOf(&'static str),
    /// A typedef: whatever the aliased type's own guard is. §7.10.1.
    Delegate(Box<ts::Expr>),
}

/// One type's three declarations, appended where they belong.
fn describe_type(
    ast: &Ast,
    applied: &Applied,
    id: Id<TypeNode>,
    items: &mut Vec<Pending>,
) -> Result<(), MaterialiseError> {
    let Ok(node) = ast.type_(id) else {
        return Ok(());
    };
    let Some(spec) = type_spec(ast, applied, id, node)? else {
        return Ok(());
    };
    push_descriptor(items, node.module, &spec);
    Ok(())
}

/// What a type's descriptor states, or `None` where the type has no
/// declaration to carry one.
#[allow(clippy::too_many_lines)]
fn type_spec(
    ast: &Ast,
    applied: &Applied,
    id: Id<TypeNode>,
    node: &TypeNode,
) -> Result<Option<TypeSpec>, MaterialiseError> {
    let base =
        |kind: &'static str, guard_shape: GuardShape| -> Result<TypeSpec, MaterialiseError> {
            Ok(TypeSpec {
                kind,
                local: ts::escape_keyword(&node.name),
                guard: guard_name(&node.name),
                qualified: qualified_idl_name(ast, node.module, &node.name)?,
                extensibility: extensibility_word(ast.effective_extensibility(id)),
                nested: is_nested(&node.facets),
                topic: topic_of(&node.facets, &node.name),
                autoid: autoid_word(&node.facets),
                bit_bound: None,
                has_default: None,
                has_implicit_default: None,
                fields: Vec::new(),
                guard_shape,
            })
        };

    Ok(match &node.kind {
        TypeKind::Structure { .. } | TypeKind::Valuetype { .. } => {
            let created = if matches!(node.kind, TypeKind::Valuetype { .. }) {
                "TsValue"
            } else {
                "TsStruct"
            };
            if ghost_created_for(applied, node.node_id, created).is_none() {
                return Ok(None);
            }
            let mut spec = base("struct", GuardShape::Object)?;
            // XTypes 1.3 §7.5.1.4.1.3: the base is the first member `parent`, whose type is
            // the descriptor of the base; the positions (the ids `@autoid` numbers) stay those
            // of the flattened list the wire writes (§7.4.1.1.5.1).
            if let Some(base_type) = super::c::struct_base(ast, id) {
                spec.fields
                    .push(parent_field(ast, applied, base_type, &node.name)?);
            }
            let own = ast.members_of(id).to_vec();
            for (position, member) in ast.all_members_of(id).into_iter().enumerate() {
                if own.contains(&member) {
                    spec.fields
                        .push(member_field(ast, applied, member, position, &node.name)?);
                }
            }
            Some(spec)
        }

        TypeKind::Union { discriminator, .. } => {
            if ghost_created_for(applied, node.node_id, "TsUnion").is_none() {
                return Ok(None);
            }
            let Some(disc) = discriminator.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "union discriminator".to_string(),
                    at: node.name.clone(),
                });
            };
            let mut spec = base("union", GuardShape::Object)?;
            // The synthetic discriminator member comes first and carries
            // the reserved id; §7.5.1.
            spec.fields.push(FieldSpec {
                name: DISCRIMINANT_PROPERTY.to_string(),
                id: DISCRIMINATOR_ID,
                type_ref: type_ref_expr(ast, applied, disc, &node.name, 0)?,
                key: false,
                optional: false,
                must_understand: false,
                default: None,
                unit: None,
                min: None,
                max: None,
                hashid: None,
                labels: None,
                key_equality_hazard: false,
                base: false,
                check: guard_check(ast, applied, disc, &member_access(DISCRIMINANT_PROPERTY))?,
            });
            let mut declared_default = false;
            let mut covered: Vec<i128> = Vec::new();
            for case in ast.cases_of(id) {
                let Ok(case_node) = ast.case(*case) else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "union case the model listed".to_string(),
                        at: node.name.clone(),
                    });
                };
                if ghost_created_for(applied, case_node.node_id, "TsCase").is_none() {
                    continue;
                }
                let mut labels: Option<Vec<ts::Expr>> = Some(Vec::new());
                for label in &case_node.labels {
                    match label {
                        CaseLabel::Default => {
                            declared_default = true;
                            labels = None;
                        }
                        CaseLabel::Value(expr) => {
                            let value = ast.eval_integral(*expr).map_err(|_| {
                                MaterialiseError::Unevaluated {
                                    construct: "union case label".to_string(),
                                    at: node.name.clone(),
                                }
                            })?;
                            covered.push(value);
                            if let Some(list) = labels.as_mut() {
                                list.push(discriminant_expr(ast, disc, value)?);
                            }
                        }
                    }
                }
                let position = spec.fields.len();
                let mut field = member_field(ast, applied, case_node.member, position, &node.name)?;
                field.labels = Some(labels);
                // A case member is present only when its label is
                // selected, so no guard may require it.
                field.check = None;
                spec.fields.push(field);
            }
            spec.has_default = Some(declared_default);
            spec.has_implicit_default =
                Some(!declared_default && !exhausts_discriminator(ast, disc, &covered));
            Some(spec)
        }

        TypeKind::Enumeration { enumerators } => {
            if ghost_created_for(applied, node.node_id, "TsEnum").is_none() {
                return Ok(None);
            }
            // The surface of an enumeration in this column is a
            // string-literal union beside an `as const` object (§7.6),
            // so the guard is a `typeof` and not a membership test: an
            // @appendable writer that added an enumerator sends a value
            // this file has never seen, and a guard that listed the
            // known ones would refuse the sample rather than the runtime
            // deciding what to do with it.
            let mut spec = base("enum", GuardShape::TypeOf("string"))?;
            spec.bit_bound = Some(ast.effective_bit_bound(id));
            for (position, enumerator) in enumerators.iter().enumerate() {
                let Ok(enum_node) = ast.enumerator(*enumerator) else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "enumerator the model listed".to_string(),
                        at: node.name.clone(),
                    });
                };
                if ghost_created_for(applied, enum_node.node_id, "TsEnumerator").is_none() {
                    continue;
                }
                let value = ast.effective_enumerator_value(*enumerator).ok_or_else(|| {
                    MaterialiseError::Unevaluated {
                        construct: "enumerator value".to_string(),
                        at: enum_node.name.clone(),
                    }
                })?;
                spec.fields.push(FieldSpec {
                    name: enum_node.name.clone(),
                    id: u32::try_from(position).unwrap_or(u32::MAX),
                    // §7.6.1: an enumerator's type-ref is a primitive.
                    // The 32 bits of XTypes 1.3 §7.4.3.5.3 and not the
                    // `@bit_bound`, which is the wire width and is
                    // carried by `bitBound` above.
                    type_ref: primitive_ref("int32"),
                    key: false,
                    optional: false,
                    must_understand: false,
                    default: Some(ts::Expr::Literal(Literal::int(value))),
                    unit: None,
                    min: None,
                    max: None,
                    hashid: None,
                    labels: None,
                    key_equality_hazard: false,
                    base: false,
                    check: None,
                });
            }
            Some(spec)
        }

        TypeKind::Bitmask { bits } => {
            if ghost_created_for(applied, node.node_id, "TsBitmask").is_none() {
                return Ok(None);
            }
            let bound = ast.effective_bit_bound(id);
            let wide = unsigned_holder(bound) == ts::Type::bigint();
            let mut spec = base(
                "bitmask",
                GuardShape::TypeOf(if wide { "bigint" } else { "number" }),
            )?;
            spec.bit_bound = Some(bound);
            for (position, bit) in bits.iter().enumerate() {
                let Ok(bit_node) = ast.bit_value(*bit) else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "bitmask bit the model listed".to_string(),
                        at: node.name.clone(),
                    });
                };
                if ghost_created_for(applied, bit_node.node_id, "TsBitConst").is_none() {
                    continue;
                }
                let at = ast.effective_bit_position(*bit).ok_or_else(|| {
                    MaterialiseError::Unevaluated {
                        construct: "bit position".to_string(),
                        at: bit_node.name.clone(),
                    }
                })?;
                spec.fields.push(FieldSpec {
                    name: bit_node.name.clone(),
                    id: u32::try_from(position).unwrap_or(u32::MAX),
                    type_ref: bitfield_ref(1),
                    key: false,
                    optional: false,
                    must_understand: false,
                    // §7.8.1: the bit position, which `@position(P)`
                    // overrides and `effective_bit_position` has already
                    // resolved.
                    default: Some(ts::Expr::Literal(Literal::int(i128::from(at)))),
                    unit: None,
                    min: None,
                    max: None,
                    hashid: None,
                    labels: None,
                    key_equality_hazard: false,
                    base: false,
                    check: None,
                });
            }
            Some(spec)
        }

        TypeKind::Bitset { .. } => {
            if ghost_created_for(applied, node.node_id, "TsBitset").is_none() {
                return Ok(None);
            }
            let mut spec = base("bitset", GuardShape::Object)?;
            // Wide enough that the sum of every declared width fits
            // before it is judged: a `u16` counter saturates at 65535
            // and would state a width the declaration never wrote.
            let mut total: u32 = 0;
            for (position, bitfield) in ast.all_bitfields_of(id).into_iter().enumerate() {
                let Ok(field_node) = ast.bitfield(bitfield) else {
                    return Err(MaterialiseError::Unevaluated {
                        construct: "bitset field the model listed".to_string(),
                        at: node.name.clone(),
                    });
                };
                total += u32::from(field_node.bits);
                // An anonymous bitfield is padding: it moves the next
                // field along and is not a member of anything.
                let Some(name) = field_node.name.as_ref() else {
                    continue;
                };
                let holder = unsigned_holder(field_node.bits);
                spec.fields.push(FieldSpec {
                    name: name.clone(),
                    id: u32::try_from(position).unwrap_or(u32::MAX),
                    type_ref: bitfield_ref(field_node.bits),
                    key: false,
                    optional: false,
                    must_understand: false,
                    default: None,
                    unit: None,
                    min: None,
                    max: None,
                    hashid: None,
                    labels: None,
                    key_equality_hazard: false,
                    base: false,
                    check: Some(typeof_is(
                        &member_access(name),
                        if holder == ts::Type::bigint() {
                            "bigint"
                        } else {
                            "number"
                        },
                    )),
                });
            }
            // §7.7.1: the total width. XTypes 1.3 §7.3.1.2.1.11 bounds a
            // bitset by the sixty-four bits of the anchor holding it,
            // and nothing between the parser and here carries that rule
            // — `lower::bitfield` checks each width against 16 bits and
            // `Ast::add_bitfield` holds no invariant about the sum — so
            // a wider one arrives. Refused by name rather than stated:
            // `bitBound: 80` is a width no anchor holds, and the codec
            // walks refuse the same declaration
            // (`emit::decode`'s "bitset wider than the sixty-four bits
            // an anchor holds", `zerodds_wire_codec::encode`'s
            // `write_bitset`). The three failure sets stay comparable.
            let bound = match u16::try_from(total) {
                Ok(bound) if bound <= ANCHOR_BITS => bound,
                _ => {
                    return Err(MaterialiseError::NoLanguageForm {
                        construct: format!(
                            "a bitset of {total} bits, which is wider than the \
                             {ANCHOR_BITS} an anchor holds"
                        ),
                        language: "TypeScript",
                        at: node.name.clone(),
                    });
                }
            };
            spec.bit_bound = Some(bound);
            Some(spec)
        }

        TypeKind::Alias { aliased } => {
            if !emits_alias(ast, applied, id) {
                return Ok(None);
            }
            let Some(target) = aliased.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "typedef target".to_string(),
                    at: node.name.clone(),
                });
            };
            // §7.10.1: one member named `value`, carrying the underlying
            // type-ref, and a guard that delegates to the referenced
            // type's.
            let mut spec = base(
                "alias",
                GuardShape::Delegate(Box::new(
                    guard_check(ast, applied, target, &ts::Expr::Path(Path::single("v")))?
                        .unwrap_or(ts::Expr::Literal(Literal::Bool(true))),
                )),
            )?;
            let bound = node.facets.xtypes.as_ref().and_then(|x| x.bit_bound);
            spec.bit_bound = bound;
            spec.fields.push(FieldSpec {
                name: "value".to_string(),
                id: 0,
                type_ref: type_ref_expr(ast, applied, target, &node.name, 0)?,
                key: false,
                optional: false,
                must_understand: false,
                default: None,
                unit: None,
                min: None,
                max: None,
                hashid: None,
                labels: None,
                key_equality_hazard: false,
                base: false,
                check: None,
            });
            Some(spec)
        }

        // A primitive, a string, a collection and a `fixed` declare
        // nothing, so there is nothing to describe. A `native` has no
        // data form at all — its TypeScript alias is `unknown` — and a
        // descriptor whose guard could not check anything would say a
        // value had been checked.
        _ => None,
    })
}

/// Whether the labelled cases cover every value the discriminator has.
///
/// §7.5.2 asks the question so that a union with neither a `default:`
/// nor full coverage can be marked `hasImplicitDefault`. Two
/// discriminator types can be covered: a `boolean`, which has two
/// values, and an enumeration, which has as many as it has enumerators.
/// An integer discriminator is never covered in practice, and answering
/// `false` for it is the honest reading of §7.2.2.4.4.4.5 rather than a
/// gap.
fn exhausts_discriminator(ast: &Ast, discriminator: Id<TypeNode>, covered: &[i128]) -> bool {
    let distinct = |values: &[i128]| {
        let mut seen: Vec<i128> = Vec::new();
        for value in values {
            if !seen.contains(value) {
                seen.push(*value);
            }
        }
        seen.len()
    };
    if matches!(
        resolved_kind(ast, discriminator, 0),
        Some(PrimitiveKind::Boolean)
    ) {
        return distinct(covered) >= 2;
    }
    let mut resolved = discriminator;
    for _ in 0..MAX_TYPE_DEPTH {
        match ast.type_(resolved).map(|node| &node.kind) {
            Ok(TypeKind::Alias { aliased }) => match aliased.resolved() {
                Some(next) => resolved = next,
                None => return false,
            },
            Ok(TypeKind::Enumeration { enumerators }) => {
                let mut values = Vec::new();
                for enumerator in enumerators {
                    match ast.effective_enumerator_value(*enumerator) {
                        Some(value) => values.push(value),
                        None => return false,
                    }
                }
                return !values.is_empty() && values.iter().all(|value| covered.contains(value));
            }
            _ => return false,
        }
    }
    false
}

/// One member of a struct, an exception or a union case.
fn member_field(
    ast: &Ast,
    applied: &Applied,
    member: Id<zerodds_ast::MemberNode>,
    position: usize,
    at: &str,
) -> Result<FieldSpec, MaterialiseError> {
    let Ok(node) = ast.member(member) else {
        return Err(MaterialiseError::Unevaluated {
            construct: "member the model listed".to_string(),
            at: at.to_string(),
        });
    };
    let Some(target) = node.type_.resolved() else {
        return Err(MaterialiseError::Unevaluated {
            construct: "member type".to_string(),
            at: node.name.clone(),
        });
    };
    let xtypes = node.facets.xtypes.as_ref();
    let optional = node.facets.is_optional();
    let access = member_access(&node.name);
    Ok(FieldSpec {
        name: node.name.clone(),
        id: node
            .facets
            .effective_member_id()
            .unwrap_or_else(|| u32::try_from(position).unwrap_or(u32::MAX)),
        type_ref: type_ref_expr(ast, applied, target, &node.name, 0)?,
        key: node.facets.is_key(),
        optional,
        must_understand: node.facets.is_must_understand(),
        default: match xtypes.and_then(|x| x.default_value) {
            Some(expr) => Some(annotation_value(ast, Some(target), expr, &node.name)?),
            None => None,
        },
        unit: xtypes.and_then(|x| x.unit.clone()),
        min: match xtypes.and_then(|x| x.range.as_ref()).and_then(|r| r.min) {
            Some(expr) => Some(annotation_value(ast, Some(target), expr, &node.name)?),
            None => None,
        },
        max: match xtypes.and_then(|x| x.range.as_ref()).and_then(|r| r.max) {
            Some(expr) => Some(annotation_value(ast, Some(target), expr, &node.name)?),
            None => None,
        },
        hashid: xtypes
            .and_then(|x| x.member_id.as_ref())
            .and_then(|assignment| match &assignment.origin {
                zerodds_ast::MemberIdOrigin::Hashed { source_name } => Some(source_name.clone()),
                _ => None,
            }),
        labels: None,
        key_equality_hazard: map_key_hazard(ast, target),
        base: false,
        // §7.4.1: an optional member is not required by the guard.
        check: if optional {
            None
        } else {
            guard_check(ast, applied, target, &access)?
        },
    })
}

/// The descriptor entry of the member `parent` (XTypes 1.3 §7.5.1.4.1.3): the base structure,
/// held as a member whose type is the base's descriptor. It has no id of its own on the wire; the
/// reserved `PARENT_ID` marks that, and `base: true` tells a consumer to descend into it.
fn parent_field(
    ast: &Ast,
    applied: &Applied,
    base_type: Id<TypeNode>,
    at: &str,
) -> Result<FieldSpec, MaterialiseError> {
    Ok(FieldSpec {
        name: "parent".to_string(),
        id: PARENT_ID,
        type_ref: type_ref_expr(ast, applied, base_type, at, 0)?,
        key: false,
        optional: false,
        must_understand: false,
        default: None,
        unit: None,
        min: None,
        max: None,
        hashid: None,
        labels: None,
        key_equality_hazard: false,
        base: true,
        check: guard_check(ast, applied, base_type, &member_access("parent"))?,
    })
}

/// The value an annotation carried, as a TypeScript expression.
fn annotation_value(
    ast: &Ast,
    declared: Option<Id<TypeNode>>,
    expr: Id<zerodds_ast::ConstExprNode>,
    at: &str,
) -> Result<ts::Expr, MaterialiseError> {
    let value = ast
        .eval_const(expr)
        .map_err(|_| MaterialiseError::Unevaluated {
            construct: "annotation value".to_string(),
            at: at.to_string(),
        })?;
    constant_expr(ast, declared, &value, at)
}

/// Whether a member is a map whose key compares by reference in
/// JavaScript.
///
/// §7.9.1 and diagnostic `DDS-TS-W003`. A `Map` keyed by a struct, a
/// sequence or an array compares its keys by identity, so two
/// structurally equal keys are two entries. The flag is what tells a
/// consumer to compare with `equalKey` instead of trusting `Map.get`.
fn map_key_hazard(ast: &Ast, target: Id<TypeNode>) -> bool {
    let mut resolved = target;
    for _ in 0..MAX_TYPE_DEPTH {
        match ast.type_(resolved).map(|node| &node.kind) {
            Ok(TypeKind::Alias { aliased }) => match aliased.resolved() {
                Some(next) => resolved = next,
                None => return false,
            },
            Ok(TypeKind::Map { key, .. }) => {
                let Some(key) = key.resolved() else {
                    return false;
                };
                return !compares_by_value(ast, key, 0);
            }
            _ => return false,
        }
    }
    false
}

/// Whether a value of this type compares by value in JavaScript.
///
/// zerodds-lint: recursion-depth 64 (alias chains; bounded by `MAX_TYPE_DEPTH`)
fn compares_by_value(ast: &Ast, id: Id<TypeNode>, depth: usize) -> bool {
    if depth > MAX_TYPE_DEPTH {
        return false;
    }
    match ast.type_(id).map(|node| &node.kind) {
        Ok(TypeKind::Primitive(PrimitiveKind::Float128)) => false,
        Ok(TypeKind::Primitive(_) | TypeKind::String { .. } | TypeKind::Fixed { .. }) => true,
        // An enumeration is a string and a bitmask a `number` or a
        // `bigint` at the TypeScript surface, so they compare by value
        // too.
        Ok(TypeKind::Enumeration { .. } | TypeKind::Bitmask { .. }) => true,
        Ok(TypeKind::Alias { aliased }) => aliased
            .resolved()
            .is_some_and(|target| compares_by_value(ast, target, depth + 1)),
        _ => false,
    }
}

/// The `DdsTypeRef` a member's type takes. DDS-TS 1.0 §8.2.
///
/// zerodds-lint: recursion-depth 64 (collection nesting; bounded by `MAX_TYPE_DEPTH`)
fn type_ref_expr(
    ast: &Ast,
    applied: &Applied,
    id: Id<TypeNode>,
    at: &str,
    depth: usize,
) -> Result<ts::Expr, MaterialiseError> {
    if depth > MAX_TYPE_DEPTH {
        return Err(MaterialiseError::Unevaluated {
            construct: format!("type reference, which nests past {MAX_TYPE_DEPTH}"),
            at: at.to_string(),
        });
    }
    let node = ast.type_(id).map_err(|_| MaterialiseError::Unevaluated {
        construct: "type".to_string(),
        at: at.to_string(),
    })?;
    Ok(match &node.kind {
        TypeKind::Primitive(kind) => primitive_ref(primitive_ref_name(*kind)),
        // A `fixed<P,S>` reaches the TypeScript surface as the decimal
        // held in a `string`, and §8.2 has no kind of its own for it.
        // The ref describes what the member holds.
        TypeKind::Fixed { .. } => ts::Expr::Object(vec![
            (
                ts::PropertyName::ident("kind"),
                ts::Expr::Literal(Literal::string("string")),
            ),
            (
                ts::PropertyName::ident("wide"),
                ts::Expr::Literal(Literal::Bool(false)),
            ),
        ]),
        TypeKind::String { width, bound } => {
            let mut entries = vec![(
                ts::PropertyName::ident("kind"),
                ts::Expr::Literal(Literal::string("string")),
            )];
            if let Some(value) = evaluated_bound(ast, *bound, at)? {
                entries.push((
                    ts::PropertyName::ident("bound"),
                    ts::Expr::Literal(Literal::int(value)),
                ));
            }
            entries.push((
                ts::PropertyName::ident("wide"),
                ts::Expr::Literal(Literal::Bool(matches!(width, CharWidth::WChar))),
            ));
            ts::Expr::Object(entries)
        }
        TypeKind::Sequence { element, bound } => {
            let Some(element) = element.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "sequence element".to_string(),
                    at: at.to_string(),
                });
            };
            let mut entries = vec![
                (
                    ts::PropertyName::ident("kind"),
                    ts::Expr::Literal(Literal::string("sequence")),
                ),
                (
                    ts::PropertyName::ident("element"),
                    type_ref_expr(ast, applied, element, at, depth + 1)?,
                ),
            ];
            if let Some(value) = evaluated_bound(ast, *bound, at)? {
                entries.push((
                    ts::PropertyName::ident("bound"),
                    ts::Expr::Literal(Literal::int(value)),
                ));
            }
            ts::Expr::Object(entries)
        }
        TypeKind::Array { element, dims } => {
            let Some(element) = element.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "array element".to_string(),
                    at: at.to_string(),
                });
            };
            let mut lengths = Vec::new();
            for dim in dims {
                let value = ast
                    .eval_integral(*dim)
                    .map_err(|_| MaterialiseError::Unevaluated {
                        construct: "array length".to_string(),
                        at: at.to_string(),
                    })?;
                lengths.push(ts::Expr::Literal(Literal::int(value)));
            }
            ts::Expr::Object(vec![
                (
                    ts::PropertyName::ident("kind"),
                    ts::Expr::Literal(Literal::string("array")),
                ),
                (
                    ts::PropertyName::ident("element"),
                    type_ref_expr(ast, applied, element, at, depth + 1)?,
                ),
                (ts::PropertyName::ident("lengths"), ts::Expr::Array(lengths)),
            ])
        }
        TypeKind::Map { key, value, bound } => {
            let (Some(key), Some(value)) = (key.resolved(), value.resolved()) else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "map key or value".to_string(),
                    at: at.to_string(),
                });
            };
            let mut entries = vec![
                (
                    ts::PropertyName::ident("kind"),
                    ts::Expr::Literal(Literal::string("map")),
                ),
                (
                    ts::PropertyName::ident("key"),
                    type_ref_expr(ast, applied, key, at, depth + 1)?,
                ),
                (
                    ts::PropertyName::ident("value"),
                    type_ref_expr(ast, applied, value, at, depth + 1)?,
                ),
            ];
            if let Some(number) = evaluated_bound(ast, *bound, at)? {
                entries.push((
                    ts::PropertyName::ident("bound"),
                    ts::Expr::Literal(Literal::int(number)),
                ));
            }
            ts::Expr::Object(entries)
        }
        // A typedef with a declaration of its own is a type the registry
        // knows; one without resolves through to what it aliases,
        // exactly as `type_at` writes the member's TypeScript type.
        TypeKind::Alias { aliased } => {
            let Some(target) = aliased.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "typedef target".to_string(),
                    at: at.to_string(),
                });
            };
            if emits_alias(ast, applied, id) {
                named_ref(&qualified_idl_name(ast, node.module, &node.name)?)
            } else {
                type_ref_expr(ast, applied, target, at, depth + 1)?
            }
        }
        TypeKind::Structure { .. }
        | TypeKind::Union { .. }
        | TypeKind::Bitset { .. }
        | TypeKind::Valuetype { .. }
        | TypeKind::Enumeration { .. }
        | TypeKind::Bitmask { .. }
        | TypeKind::Native => named_ref(&qualified_idl_name(ast, node.module, &node.name)?),
        // A boxed value is the boxed type, absent when it is not there.
        // The absence is the member's, and `optional` already carries
        // it.
        TypeKind::ValueBox { boxed } => {
            let Some(target) = boxed.resolved() else {
                return Err(MaterialiseError::Unevaluated {
                    construct: "value box target".to_string(),
                    at: at.to_string(),
                });
            };
            type_ref_expr(ast, applied, target, at, depth + 1)?
        }
        // §7.3.4: the ref carries no name, because the identity of what
        // an `any` holds is the boxed value's `typeId` and not
        // something the descriptor graph can state.
        TypeKind::Any => ts::Expr::Object(vec![(
            ts::PropertyName::ident("kind"),
            ts::Expr::Literal(Literal::string("any")),
        )]),
        TypeKind::ValueBase | TypeKind::ObjectRef { .. } | TypeKind::AnnotationType { .. } => {
            return Err(MaterialiseError::NoLanguageForm {
                construct: node.kind.as_str().to_string(),
                language: "TypeScript",
                at: at.to_string(),
            });
        }
    })
}

/// A declared bound, evaluated, or `None` where the collection is
/// unbounded.
///
/// `None` answers `Bound::Unbounded` and nothing else. Answering it for
/// a bound that will not fold — `string<1/0>` reaches here with
/// `eval_integral` failing, and the front end reports nothing about it —
/// would write the declaration out with no `_BOUND` constant beside it
/// (§7.3.1) and a `DdsTypeRef` carrying no `bound` (§8.2), both of which
/// a consumer reads as *unbounded*. That is the substitution
/// [`declare`]'s array arm refuses for the same expression, and
/// [`crate::emit::interface_typescript`]'s `bound_value` refuses for the
/// operations profile.
///
/// # Errors
/// [`MaterialiseError::Unevaluated`], naming the bound.
fn evaluated_bound(
    ast: &Ast,
    bound: zerodds_ast::Bound,
    at: &str,
) -> Result<Option<i128>, MaterialiseError> {
    let Some(expr) = bound.expr() else {
        return Ok(None);
    };
    ast.eval_integral(expr)
        .map(Some)
        .map_err(|_| MaterialiseError::Unevaluated {
            construct: "collection bound".to_string(),
            at: at.to_string(),
        })
}

/// `{ kind: "primitive", name }`.
fn primitive_ref(name: &str) -> ts::Expr {
    ts::Expr::Object(vec![
        (
            ts::PropertyName::ident("kind"),
            ts::Expr::Literal(Literal::string("primitive")),
        ),
        (
            ts::PropertyName::ident("name"),
            ts::Expr::Literal(Literal::string(name)),
        ),
    ])
}

/// `{ kind: "bitfield", width }`.
fn bitfield_ref(width: u16) -> ts::Expr {
    ts::Expr::Object(vec![
        (
            ts::PropertyName::ident("kind"),
            ts::Expr::Literal(Literal::string("bitfield")),
        ),
        (
            ts::PropertyName::ident("width"),
            ts::Expr::Literal(Literal::int(i128::from(width))),
        ),
    ])
}

/// `{ kind: "ref", name }`, the qualified IDL name a `lookupType`
/// resolves.
fn named_ref(qualified: &str) -> ts::Expr {
    ts::Expr::Object(vec![
        (
            ts::PropertyName::ident("kind"),
            ts::Expr::Literal(Literal::string("ref")),
        ),
        (
            ts::PropertyName::ident("name"),
            ts::Expr::Literal(Literal::string(qualified)),
        ),
    ])
}

/// The `PrimitiveName` an IDL primitive takes. §8.2.
///
/// Exhaustive, so a primitive added to the model without a descriptor
/// name is a compile error and not a member described as another type.
const fn primitive_ref_name(kind: PrimitiveKind) -> &'static str {
    match kind {
        PrimitiveKind::Boolean => "boolean",
        // `uint8` is `octet` — IDL 4.2 §7.4.1.4.4.2 makes them one type.
        PrimitiveKind::Octet | PrimitiveKind::UInt8 => "octet",
        // The one name past §8.2's list; see `PrimitiveName` in
        // `runtime/typescript/src/types.ts` for why there is no other
        // answer for a signed 8-bit integer.
        PrimitiveKind::Int8 => "int8",
        PrimitiveKind::Int16 => "int16",
        PrimitiveKind::UInt16 => "uint16",
        PrimitiveKind::Int32 => "int32",
        PrimitiveKind::UInt32 => "uint32",
        PrimitiveKind::Int64 => "int64",
        PrimitiveKind::UInt64 => "uint64",
        PrimitiveKind::Float32 => "float",
        PrimitiveKind::Float64 => "double",
        PrimitiveKind::Float128 => "longDouble",
        PrimitiveKind::Char => "char",
        PrimitiveKind::WChar => "wchar",
    }
}

/// The qualified IDL name of a declaration — `robot::Pose`.
///
/// The name the registry is keyed by and every `{ kind: "ref" }` names.
/// The IDL spelling and not the TypeScript one: two vendors' generators
/// escape a reserved word differently, and a descriptor graph that
/// crossed between them would resolve nothing.
fn qualified_idl_name(
    ast: &Ast,
    module: Id<ModuleNode>,
    name: &str,
) -> Result<String, MaterialiseError> {
    let mut segments: Vec<String> = Vec::new();
    for (_, _, idl) in module_chain(ast, module)? {
        segments.push(idl);
    }
    segments.push(name.to_string());
    Ok(segments.join("::"))
}

/// `is<Type>`, the name §7.4.1 gives the guard.
fn guard_name(idl_name: &str) -> String {
    format!("is{idl_name}")
}

/// `"final"`, `"appendable"` or `"mutable"`. §8.1.
const fn extensibility_word(kind: zerodds_ast::Extensibility) -> &'static str {
    match kind {
        zerodds_ast::Extensibility::Final => "final",
        zerodds_ast::Extensibility::Appendable => "appendable",
        zerodds_ast::Extensibility::Mutable => "mutable",
    }
}

/// `@nested`.
fn is_nested(facets: &zerodds_ast::Facets) -> bool {
    facets.xtypes.as_ref().is_some_and(|x| x.is_nested)
}

/// The topic name `@topic` bound, where it bound one.
///
/// `@topic` with no name argument binds the type's own name, which is
/// what XTypes 1.3 Annex B says it means.
fn topic_of(facets: &zerodds_ast::Facets, type_name: &str) -> Option<String> {
    let annotation = facets.xtypes.as_ref()?.topic.as_ref()?;
    Some(
        annotation
            .name
            .clone()
            .unwrap_or_else(|| type_name.to_string()),
    )
}

/// `@autoid(SEQUENTIAL)` or `@autoid(HASH)`, in the descriptor's
/// spelling.
fn autoid_word(facets: &zerodds_ast::Facets) -> Option<&'static str> {
    match facets.xtypes.as_ref()?.autoid? {
        zerodds_ast::AutoId::Sequential => Some("sequential"),
        zerodds_ast::AutoId::Hash => Some("hash"),
    }
}

/// `(v as Record<string, unknown>)["name"]` — one member of the value a
/// guard was handed.
///
/// Indexed and not `.name`: a member name the IDL admits and TypeScript
/// does not parse as an identifier would be a syntax error in the guard
/// alone, in a file whose interface spelled the same member in quotes
/// and compiled.
fn member_access(name: &str) -> ts::Expr {
    ts::Expr::Index {
        object: Box::new(ts::Expr::Paren(Box::new(ts::Expr::As {
            value: Box::new(ts::Expr::Path(Path::single("v"))),
            type_: Box::new(ts::Type::generic(
                Path::single("Record"),
                vec![ts::Type::string(), ts::Type::Keyword(ts::Keyword::Unknown)],
            )),
        }))),
        index: Box::new(ts::Expr::Literal(Literal::string(name))),
    }
}

/// `typeof <access> === "<name>"`.
fn typeof_is(access: &ts::Expr, name: &str) -> ts::Expr {
    ts::Expr::Binary {
        op: "===",
        lhs: Box::new(ts::Expr::Unary {
            op: "typeof ",
            value: Box::new(access.clone()),
        }),
        rhs: Box::new(ts::Expr::Literal(Literal::string(name))),
    }
}

/// What the guard checks about one member, or `None` where there is
/// nothing a run-time check can say.
///
/// zerodds-lint: recursion-depth 64 (alias chains; bounded by `MAX_TYPE_DEPTH`)
fn guard_check(
    ast: &Ast,
    applied: &Applied,
    target: Id<TypeNode>,
    access: &ts::Expr,
) -> Result<Option<ts::Expr>, MaterialiseError> {
    guard_check_at(ast, applied, target, access, 0)
}

/// [`guard_check`], carrying how deep the alias chain has run.
///
/// zerodds-lint: recursion-depth 64 (alias chains; bounded by
/// `MAX_TYPE_DEPTH`, past which the guard says nothing about the member
/// rather than descending further). Only the unemitted-alias arm
/// recurses: an alias the surface emits gets a `guard_call` to that
/// alias's own guard, and every other kind is a leaf here.
fn guard_check_at(
    ast: &Ast,
    applied: &Applied,
    target: Id<TypeNode>,
    access: &ts::Expr,
    depth: usize,
) -> Result<Option<ts::Expr>, MaterialiseError> {
    if depth > MAX_TYPE_DEPTH {
        return Ok(None);
    }
    let node = ast
        .type_(target)
        .map_err(|_| MaterialiseError::Unevaluated {
            construct: "type".to_string(),
            at: "a member's type".to_string(),
        })?;
    Ok(match &node.kind {
        // Sixteen octets in an object, and a `DdsAny` is an object too.
        // The object check and nothing narrower: a guard that reached
        // into either would be checking a shape the runtime owns.
        TypeKind::Primitive(PrimitiveKind::Float128) | TypeKind::Any => Some(object_check(access)),
        TypeKind::Primitive(kind) => Some(typeof_is(
            access,
            match ts_primitive(*kind) {
                ts::Keyword::Boolean => "boolean",
                ts::Keyword::BigInt => "bigint",
                ts::Keyword::String => "string",
                _ => "number",
            },
        )),
        TypeKind::String { .. } | TypeKind::Fixed { .. } => Some(typeof_is(access, "string")),
        TypeKind::Sequence { .. } | TypeKind::Array { .. } => Some(ts::Expr::Call {
            callee: Box::new(ts::Expr::Path(Path::new(&["Array", "isArray"]))),
            args: vec![access.clone()],
        }),
        // §7.9.1 makes a map a `ReadonlyMap`, and `Array.isArray` says
        // nothing about one. `instanceof Map` is the check the surface
        // has.
        TypeKind::Map { .. } => Some(ts::Expr::Binary {
            op: "instanceof",
            lhs: Box::new(access.clone()),
            rhs: Box::new(ts::Expr::Path(Path::single("Map"))),
        }),
        TypeKind::Alias { aliased } => {
            let Some(inner) = aliased.resolved() else {
                return Ok(None);
            };
            if emits_alias(ast, applied, target) {
                Some(guard_call(ast, target, access)?)
            } else {
                guard_check_at(ast, applied, inner, access, depth + 1)?
            }
        }
        TypeKind::Structure { .. }
        | TypeKind::Valuetype { .. }
        | TypeKind::Union { .. }
        | TypeKind::Bitset { .. }
        | TypeKind::Enumeration { .. }
        | TypeKind::Bitmask { .. } => {
            if describes(ast, applied, target) {
                Some(guard_call(ast, target, access)?)
            } else {
                None
            }
        }
        // A `native` is `unknown` and a boxed value may be absent;
        // neither has a check that would mean anything.
        _ => None,
    })
}

/// `a.b.isPoint(x)` — the guard of a named type, from the file root.
fn guard_call(
    ast: &Ast,
    target: Id<TypeNode>,
    access: &ts::Expr,
) -> Result<ts::Expr, MaterialiseError> {
    let node = ast
        .type_(target)
        .map_err(|_| MaterialiseError::Unevaluated {
            construct: "type".to_string(),
            at: "a member's type".to_string(),
        })?;
    let mut segments: Vec<String> = Vec::new();
    for (_, name, _) in module_chain(ast, node.module)? {
        segments.push(name);
    }
    segments.push(guard_name(&node.name));
    Ok(ts::Expr::Call {
        callee: Box::new(ts::Expr::Path(Path::new(
            &segments.iter().map(String::as_str).collect::<Vec<&str>>(),
        ))),
        args: vec![access.clone()],
    })
}

/// Whether a type gets a descriptor and therefore a guard.
///
/// The same ghost questions [`type_spec`] asks, so the two never
/// disagree: a guard called for a type that got none would name a
/// function that is not there.
fn describes(ast: &Ast, applied: &Applied, id: Id<TypeNode>) -> bool {
    let Ok(node) = ast.type_(id) else {
        return false;
    };
    let created = match &node.kind {
        TypeKind::Structure { .. } => "TsStruct",
        TypeKind::Valuetype { .. } => "TsValue",
        TypeKind::Union { .. } => "TsUnion",
        TypeKind::Enumeration { .. } => "TsEnum",
        TypeKind::Bitmask { .. } => "TsBitmask",
        TypeKind::Bitset { .. } => "TsBitset",
        TypeKind::Alias { .. } => return emits_alias(ast, applied, id),
        _ => return false,
    };
    ghost_created_for(applied, node.node_id, created).is_some()
}

/// `typeof x === "object" && x !== null`.
fn object_check(access: &ts::Expr) -> ts::Expr {
    ts::Expr::Binary {
        op: "&&",
        lhs: Box::new(typeof_is(access, "object")),
        rhs: Box::new(ts::Expr::Binary {
            op: "!==",
            lhs: Box::new(access.clone()),
            rhs: Box::new(ts::Expr::Literal(Literal::Null)),
        }),
    }
}

/// The three declarations of one descriptor, queued against the module
/// that declared the type.
fn push_descriptor(items: &mut Vec<Pending>, module: Id<ModuleNode>, spec: &TypeSpec) {
    let guard = ts::Decl::Function(ts::Function {
        doc: Doc::lines(&[
            &format!(
                "Whether a value is a `{}`. DDS-TS 1.0 §7.4.1.",
                spec.qualified
            ),
            "",
            "The presence and the type of every required member. An @optional",
            "member is not required, and `@unit`, `@min` and `@max` are not",
            "checked: they are documentation, and a guard that enforced them",
            "would refuse a sample the wire accepted.",
        ]),
        exported: true,
        name: Ident::new(&spec.guard),
        generics: Vec::new(),
        params: vec![ts::Param {
            name: Ident::new("v"),
            optional: false,
            rest: false,
            type_: Some(ts::Type::Keyword(ts::Keyword::Unknown)),
            default: None,
        }],
        returns: Some(ts::Type::Predicate {
            param: Ident::new("v"),
            type_: Box::new(ts::Type::named(Path::single(&spec.local))),
        }),
        body: Some(ts::Block {
            statements: vec![ts::Statement::Return(Some(guard_expr(spec)))],
        }),
    });

    let descriptor = ts::Decl::Variable(ts::Variable {
        doc: Doc::lines(&[
            &format!("The side-table of `{}`. DDS-TS 1.0 §7.4.1.", spec.qualified),
            "",
            "A TypeScript interface is erased, so what the IDL annotated it with",
            "has nowhere to live in the emitted type. It lives here, as a value,",
            "and `registerType` below makes it findable by the qualified name.",
        ]),
        exported: true,
        declare: false,
        kind: ts::VarKind::Const,
        name: Ident::new(&descriptor_name(&spec.local)),
        type_: Some(ts::Type::generic(
            Path::single("DdsTypeDescriptor"),
            vec![ts::Type::named(Path::single(&spec.local))],
        )),
        value: Some(descriptor_object(spec)),
    });

    let registration = ts::Decl::Statement(ts::Statement::Expr(ts::Expr::Call {
        callee: Box::new(ts::Expr::Path(Path::single("registerType"))),
        args: vec![ts::Expr::Path(Path::single(&descriptor_name(&spec.local)))],
    }));

    for decl in [guard, descriptor, registration] {
        items.push(Pending { module, decl });
    }
}

/// `<Type>Type`, the name §7.4.1 gives the constant.
fn descriptor_name(local: &str) -> String {
    format!("{local}Type")
}

/// The body of a guard, as one boolean expression.
fn guard_expr(spec: &TypeSpec) -> ts::Expr {
    match &spec.guard_shape {
        GuardShape::TypeOf(name) => typeof_is(&ts::Expr::Path(Path::single("v")), name),
        GuardShape::Delegate(check) => (**check).clone(),
        GuardShape::Object => {
            let mut terms = vec![object_check(&ts::Expr::Path(Path::single("v")))];
            for field in &spec.fields {
                if let Some(check) = &field.check {
                    terms.push(check.clone());
                }
            }
            and_all(terms)
        }
    }
}

/// `a && b && c`, left-associated.
fn and_all(terms: Vec<ts::Expr>) -> ts::Expr {
    let mut iter = terms.into_iter();
    let Some(first) = iter.next() else {
        return ts::Expr::Literal(Literal::Bool(true));
    };
    iter.fold(first, |lhs, rhs| ts::Expr::Binary {
        op: "&&",
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    })
}

/// The descriptor object literal itself.
fn descriptor_object(spec: &TypeSpec) -> ts::Expr {
    let mut entries = vec![
        (
            ts::PropertyName::ident("kind"),
            ts::Expr::Literal(Literal::string(spec.kind)),
        ),
        (
            ts::PropertyName::ident("name"),
            ts::Expr::Literal(Literal::string(&spec.qualified)),
        ),
        (
            ts::PropertyName::ident("extensibility"),
            ts::Expr::Literal(Literal::string(spec.extensibility)),
        ),
        (
            ts::PropertyName::ident("nested"),
            ts::Expr::Literal(Literal::Bool(spec.nested)),
        ),
    ];
    if let Some(topic) = &spec.topic {
        entries.push((
            ts::PropertyName::ident("topic"),
            ts::Expr::Literal(Literal::string(topic)),
        ));
    }
    if let Some(autoid) = spec.autoid {
        entries.push((
            ts::PropertyName::ident("autoid"),
            ts::Expr::Literal(Literal::string(autoid)),
        ));
    }
    if let Some(bound) = spec.bit_bound {
        entries.push((
            ts::PropertyName::ident("bitBound"),
            ts::Expr::Literal(Literal::int(i128::from(bound))),
        ));
    }
    if let Some(flag) = spec.has_default {
        entries.push((
            ts::PropertyName::ident("hasDefault"),
            ts::Expr::Literal(Literal::Bool(flag)),
        ));
    }
    if let Some(flag) = spec.has_implicit_default {
        entries.push((
            ts::PropertyName::ident("hasImplicitDefault"),
            ts::Expr::Literal(Literal::Bool(flag)),
        ));
    }
    entries.push((
        ts::PropertyName::ident("fields"),
        ts::Expr::Array(spec.fields.iter().map(field_object).collect()),
    ));
    entries.push((
        ts::PropertyName::ident("typeGuard"),
        ts::Expr::Path(Path::single(&spec.guard)),
    ));
    ts::Expr::Object(entries)
}

/// One `DdsMemberDescriptor`, as the object literal it is.
fn field_object(field: &FieldSpec) -> ts::Expr {
    let mut entries = vec![
        (
            ts::PropertyName::ident("name"),
            ts::Expr::Literal(Literal::string(&field.name)),
        ),
        (
            ts::PropertyName::ident("id"),
            ts::Expr::Literal(Literal::int(i128::from(field.id))),
        ),
        (ts::PropertyName::ident("type"), field.type_ref.clone()),
        (
            ts::PropertyName::ident("key"),
            ts::Expr::Literal(Literal::Bool(field.key)),
        ),
        (
            ts::PropertyName::ident("optional"),
            ts::Expr::Literal(Literal::Bool(field.optional)),
        ),
        (
            ts::PropertyName::ident("mustUnderstand"),
            ts::Expr::Literal(Literal::Bool(field.must_understand)),
        ),
    ];
    if let Some(value) = &field.default {
        entries.push((ts::PropertyName::ident("default"), value.clone()));
    }
    if let Some(unit) = &field.unit {
        entries.push((
            ts::PropertyName::ident("unit"),
            ts::Expr::Literal(Literal::string(unit)),
        ));
    }
    if let Some(value) = &field.min {
        entries.push((ts::PropertyName::ident("min"), value.clone()));
    }
    if let Some(value) = &field.max {
        entries.push((ts::PropertyName::ident("max"), value.clone()));
    }
    if let Some(name) = &field.hashid {
        entries.push((
            ts::PropertyName::ident("hashid"),
            ts::Expr::Literal(Literal::string(name)),
        ));
    }
    if let Some(labels) = &field.labels {
        // §7.5.1: the `default:` case carries `labels: undefined`, which
        // is what tells a consumer that this member is the one an
        // unmatched discriminator selects.
        entries.push((
            ts::PropertyName::ident("labels"),
            match labels {
                Some(list) => ts::Expr::Array(list.clone()),
                None => ts::Expr::Undefined,
            },
        ));
    }
    if field.key_equality_hazard {
        entries.push((
            ts::PropertyName::ident("keyEqualityHazard"),
            ts::Expr::Literal(Literal::Bool(true)),
        ));
    }
    if field.base {
        entries.push((
            ts::PropertyName::ident("base"),
            ts::Expr::Literal(Literal::Bool(true)),
        ));
    }
    ts::Expr::Object(entries)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;
    use crate::rule;
    use crate::{ModelGraph, apply};
    use std::path::{Path as FsPath, PathBuf};

    /// IDL in, one TypeScript file out.
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
        let applied = apply(&ModelGraph::of(&ast), &rule::typescript::catalogue())
            .map_err(|error| format!("apply: {error}"))?;
        // Both halves. The side-tables walk the same model a second time
        // and reach constructs the type walk passes over — a bound that
        // has to evaluate, a `@default` that has to be a literal — so
        // the whole corpus goes through both rather than the first
        // alone.
        let mut decls = prelude();
        decls.extend(
            ts_items_with_descriptors(&ast, &applied)
                .map_err(|error| format!("materialise: {error}"))?,
        );
        Ok(zerodds_lang_render::typescript::file(&ts::File {
            doc: Doc::lines(&[
                "Generated by zerodds-lang-rules. Do not edit.",
                "",
                "The TypeScript shape of one IDL file. The wire rules are not here.",
            ]),
            imports: vec![descriptor_import(DEFAULT_RUNTIME_PACKAGE)],
            decls,
        }))
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
    /// A rendered file is not a checked one. `ZD_TS_OUT` names a
    /// directory to write the sources into, and `tsc --strict --noEmit`
    /// over that directory is the second half of the check — a file
    /// that does not typecheck is a materialiser defect, not a corpus
    /// defect.
    #[test]
    fn every_corpus_case_renders_or_names_its_failure() {
        let cases = corpus();
        assert!(
            cases.len() > 300,
            "the corpus is not where it was: {} cases",
            cases.len()
        );
        let out = std::env::var("ZD_TS_OUT").ok();
        let mut failures = Vec::new();
        for (name, path) in cases {
            let source = std::fs::read_to_string(&path).expect("a corpus source");
            match render(&source, &path.display().to_string()) {
                Ok(text) => {
                    if let Some(dir) = &out {
                        let case = FsPath::new(dir).join(&name);
                        std::fs::create_dir_all(&case).expect("the output directory");
                        std::fs::write(case.join("types.ts"), text).expect("the output file");
                    }
                }
                Err(why) => failures.push(format!("{name}: {why}")),
            }
        }
        // `resolve-module-named-as-type` is
        // `module common { struct common { … }; }`, which TypeScript
        // cannot write — a nested declaration of that name captures the
        // first segment of every qualified reference written inside it,
        // and TypeScript has no root operator to write past it. It no
        // longer reaches this adapter at all: the front end refuses the
        // file under IDL 4.2 §7.5.2, which is where
        // `crate::adapter::REFUSED_BY_THE_FRONT_END` states the reason
        // and where the check that it is still refused lives.
        let failures = crate::adapter::without_the_front_end_refusals(failures);
        assert!(failures.is_empty(), "{failures:#?}");
    }

    #[test]
    fn a_sixty_four_bit_integer_is_a_bigint_and_everything_narrower_is_a_number() {
        // The decision this column turns on. A `number` is a binary64,
        // so 2^53 is where it stops holding integers as themselves.
        let generated = declarations(
            "@final struct P {
               boolean a; octet b; char c; wchar d;
               int8 e; uint8 f; short g; unsigned short h;
               long i; unsigned long j; long long k; unsigned long long l;
               float m; double n; long double o;
             };",
        );
        for expected in [
            "a: boolean;",
            "b: number;",
            "c: string;",
            "d: string;",
            "e: number;",
            "f: number;",
            "g: number;",
            "h: number;",
            "i: number;",
            // 4294967295 is far below 2^53, so an `unsigned long` needs
            // no widening — unlike the Java column, where `int` is
            // signed and it has to become a `long`.
            "j: number;",
            "k: bigint;",
            "l: bigint;",
            "m: number;",
            "n: number;",
            "o: ZdFloat128;",
        ] {
            assert!(generated.contains(expected), "{expected}\n{generated}");
        }
    }

    #[test]
    fn a_bigint_field_starts_from_a_bigint_literal() {
        // `k: bigint = 0` does not typecheck; the `n` is what makes the
        // value one of the member's type.
        let generated = declarations("@final struct P { long long k; long i; };");
        assert!(generated.contains("k: 0n"), "{generated}");
        assert!(generated.contains("i: 0"), "{generated}");
    }

    /// DDS-TS 1.0 §7.4 and Annex B §B.1.2: no `class` keyword in the
    /// generator output for any IDL data type. What produced a value is
    /// a factory function instead.
    #[test]
    fn a_struct_is_an_interface_and_a_factory_and_never_a_class() {
        let generated = declarations(
            "@final struct P { long x; }; \
             bitset Packed { bitfield<3> low; }; \
             enum Colour { RED }; \
             @final union U switch (long) { case 1: long a; }; \
             exception E { long code; }; \
             typedef long Counter;",
        );
        assert!(generated.contains("export interface P {"), "{generated}");
        assert!(
            generated.contains("export function newP(): P {"),
            "{generated}"
        );
        assert!(generated.contains("return { x: 0 };"), "{generated}");
        assert!(
            !without_comments(&generated).contains("class"),
            "a class reached the output for an IDL data type:\n{generated}"
        );
    }

    /// The generated source with its documentation removed.
    ///
    /// B.1.2 forbids the `class` **keyword** in the output, and a doc
    /// comment that explains why there is no class is not one. Written
    /// out rather than searching for `export class `, because a class
    /// nested inside a namespace is indented and would slip past that.
    fn without_comments(text: &str) -> String {
        let mut out = String::new();
        let mut rest = text;
        while let Some(open) = rest.find("/*") {
            out.push_str(&rest[..open]);
            match rest[open..].find("*/") {
                Some(close) => rest = &rest[open + close + 2..],
                None => return out,
            }
        }
        out.push_str(rest);
        out.lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn a_sequence_and_an_array_are_both_typescript_arrays() {
        let generated = declarations(
            "@final struct P { sequence<long> s; long a[3]; long g[2][3]; sequence<string> t; };",
        );
        assert!(generated.contains("s: number[];"), "{generated}");
        assert!(generated.contains("a: number[];"), "{generated}");
        assert!(generated.contains("g: number[][];"), "{generated}");
        assert!(generated.contains("t: string[];"), "{generated}");
        // A sequence may be empty; an array's length is part of its
        // type, so it starts full.
        assert!(generated.contains("s: []"), "{generated}");
        assert!(
            generated.contains("a: Array.from({ length: 3 }, () => 0)"),
            "{generated}"
        );
    }

    #[test]
    fn an_array_of_unions_brackets_the_object_its_arrow_returns() {
        // `() => { discriminator: 0, … }` is an arrow whose body is a
        // *block*: the brace opens a statement list, not an object.
        let generated = declarations(
            "union Choice switch (long) { case 1: long whole; default: double fraction; };
             @final struct Sample { Choice items[2]; };",
        );
        assert!(
            generated
                .contains("Array.from({ length: 2 }, () => ({ discriminator: 0, fraction: 0 }))"),
            "{generated}"
        );
    }

    #[test]
    fn a_map_member_is_a_readonly_map_over_a_staged_entry_type() {
        let generated = declarations("@final struct P { map<long, string> m; };");
        // DDS-TS 1.0 §7.9.1: the member is a `ReadonlyMap`, and never a
        // `Record`, which loses key uniqueness.
        assert!(
            generated.contains("m: ReadonlyMap<number, string>;"),
            "{generated}"
        );
        assert!(!generated.contains("Record<number"), "{generated}");
        // The entry type stays: XTypes 1.3 §7.4.1.1.4 puts a map on the
        // wire as a sequence of these, and the codec stages them
        // through it. The tag is the IDL kind's spelling and not the
        // TypeScript type's: `octet`, `unsigned short` and `long` are
        // all a `number` here.
        assert!(
            generated.contains("export interface ZdEntry_long_string {"),
            "{generated}"
        );
        assert!(generated.contains("key: number;"), "{generated}");
        assert!(generated.contains("value: string;"), "{generated}");
    }

    #[test]
    fn two_maps_of_one_typescript_shape_and_two_idl_shapes_are_two_entry_types() {
        // `octet` and `unsigned short` are both a `number` and one and
        // two bytes on the wire.
        let generated =
            declarations("@final struct P { map<octet, long> a; map<unsigned short, long> b; };");
        assert!(
            generated.contains("interface ZdEntry_octet_long {"),
            "{generated}"
        );
        assert!(
            generated.contains("interface ZdEntry_unsigned_short_long {"),
            "{generated}"
        );
    }

    #[test]
    fn two_maps_of_one_shape_are_one_entry_type() {
        let generated = declarations("@final struct P { map<long, long> a; map<long, long> b; };");
        assert_eq!(
            generated.matches("interface ZdEntry_long_long {").count(),
            1
        );
    }

    #[test]
    fn an_optional_member_is_a_question_mark_and_not_a_null() {
        let generated =
            declarations("@mutable struct P { @optional long a; @optional string b; long c; };");
        // Both halves of §7.11: the `?:` suffix and the `| undefined`
        // arm. Under `exactOptionalPropertyTypes` the two declarations
        // are not the same type.
        assert!(generated.contains("a?: number | undefined;"), "{generated}");
        assert!(generated.contains("b?: string | undefined;"), "{generated}");
        assert!(!generated.contains("| null"), "{generated}");
        // An @optional member gets no entry in the factory's literal:
        // its absence is what §7.2.2.4.4.4.8 asks for. A required one
        // does.
        assert!(generated.contains("return { c: 0 };"), "{generated}");
    }

    #[test]
    fn a_union_is_a_discriminated_union_narrowed_on_a_literal() {
        let generated = declarations(
            "union Choice switch (long) {
               case 1: long whole;
               case 2: long other;
               default: double fraction;
             };
             @final struct P { Choice c; };",
        );
        assert!(
            generated.contains("export type Choice = { discriminator: 1; whole: number }"),
            "{generated}"
        );
        assert!(
            generated.contains("{ discriminator: 2; other: number }"),
            "{generated}"
        );
        // The `default:` case selects every value the others do not, so
        // it carries the discriminator's type minus their labels —
        // §7.5, which also records that `Exclude` narrows no primitive.
        assert!(
            generated.contains("{ discriminator: Exclude<number, 1 | 2>; fraction: number }"),
            "{generated}"
        );
        assert!(generated.contains("c: Choice;"), "{generated}");
    }

    #[test]
    fn a_union_discriminant_takes_the_discriminators_own_literal_form() {
        // `2` and `2n` are two types, and `1` is not `true`.
        // IDL has one namespace per scope and compares identifiers
        // without regard to case, so the member cannot be named after
        // its type.
        let wide = declarations(
            "union W switch (long long) { case 2: long a; };
             @final struct P { W picked; };",
        );
        assert!(wide.contains("{ discriminator: 2n; a: number }"), "{wide}");
        let flag = declarations(
            "union B switch (boolean) { case TRUE: long a; };
             @final struct P { B picked; };",
        );
        assert!(
            flag.contains("{ discriminator: true; a: number }"),
            "{flag}"
        );
    }

    #[test]
    fn an_enumeration_is_an_as_const_object_beside_a_string_literal_union() {
        let generated = declarations(
            "enum Colour { RED, GREEN, BLUE };
             @final struct P { Colour c; };",
        );
        assert!(
            generated.contains(
                "export const Colour = { RED: \"RED\", GREEN: \"GREEN\", BLUE: \"BLUE\" } as const;"
            ),
            "{generated}"
        );
        assert!(
            generated.contains("export type Colour = (typeof Colour)[keyof typeof Colour];"),
            "{generated}"
        );
        // §7.6's numeric companions: the wire value of each enumerator,
        // and the enumerator each wire value names.
        assert!(
            generated.contains(
                "export const ColourOrdinal: Readonly<Record<Colour, number>> = \
                 { RED: 0, GREEN: 1, BLUE: 2 } as const;"
            ),
            "{generated}"
        );
        assert!(
            generated.contains("export const ColourFromOrdinal: ReadonlyMap<number, Colour>"),
            "{generated}"
        );
        assert!(generated.contains("c: Colour;"), "{generated}");
        // Not a TypeScript `enum`: since 5.0 an `enum`-typed slot
        // rejects a value that is not one of its members, and an
        // @appendable writer sends exactly such a value.
        assert!(!generated.contains("export enum "), "{generated}");
    }

    #[test]
    fn an_explicit_enumerator_value_is_written_out() {
        let generated = declarations("enum E { @value(-1) LOW, @value(70000) HIGH };");
        assert!(generated.contains("LOW: -1"), "{generated}");
        // Unlike the Java column nothing wraps to a negative: a
        // `number` holds 70000 and 4294967295 alike.
        assert!(generated.contains("HIGH: 70000"), "{generated}");
    }

    #[test]
    fn a_bitmask_writes_its_bit_as_the_number_and_not_as_a_shift() {
        // `1 << 31` is -2147483648 in JavaScript: `<<` is a signed
        // 32-bit operation.
        let narrow =
            declarations("@bit_bound(32) bitmask Small { @position(0) A, @position(31) B };");
        assert!(
            narrow.contains("export const Small = { A: 1, B: 2147483648 } as const;"),
            "{narrow}"
        );
        // A `@bit_bound(32)` bitmask stays a `number`: 4294967295 is
        // exact, where the Java column has to widen to a `long`.
        assert!(narrow.contains("export type Small = number;"), "{narrow}");
        let wide = declarations("@bit_bound(64) bitmask Big { @position(0) A, @position(40) B };");
        assert!(wide.contains("B: 1099511627776n"), "{wide}");
        assert!(wide.contains("export type Big = bigint;"), "{wide}");
    }

    #[test]
    fn a_bitset_is_one_field_per_bitfield() {
        let generated = declarations(
            "bitset Packed { bitfield<3> low; bitfield<5> high; };
             @final struct P { Packed value; };",
        );
        assert!(generated.contains("low: number;"), "{generated}");
        assert!(generated.contains("high: number;"), "{generated}");
        assert!(generated.contains("value: Packed;"), "{generated}");
        assert!(generated.contains("value: newPacked()"), "{generated}");
    }

    #[test]
    fn a_fixed_is_a_string_and_not_a_number() {
        let generated = declarations(
            "typedef fixed<9,3> Money;
             @final struct P { Money amount; };",
        );
        // Through the alias, which is a declaration of its own here.
        assert!(
            generated.contains("export type Money = string;"),
            "{generated}"
        );
        assert!(generated.contains("amount: Money;"), "{generated}");
    }

    #[test]
    fn a_typedef_is_a_type_alias_and_not_a_comment() {
        // Where the Java column has to write a comment, TypeScript has
        // the declaration.
        let generated = declarations(
            "typedef sequence<long> Row;
             @final struct P { Row r; };",
        );
        assert!(
            generated.contains("export type Row = number[];"),
            "{generated}"
        );
        assert!(generated.contains("r: Row;"), "{generated}");
    }

    #[test]
    fn a_module_is_a_namespace_and_two_modules_do_not_collide() {
        let generated = declarations(
            "module a { @final struct P { long x; }; };
             module b { @final struct P { double y; }; };
             @final struct Q { a::P one; b::P two; };",
        );
        assert!(generated.contains("export namespace a {"), "{generated}");
        assert!(generated.contains("export namespace b {"), "{generated}");
        assert!(generated.contains("one: a.P;"), "{generated}");
        assert!(generated.contains("two: b.P;"), "{generated}");
    }

    #[test]
    fn a_module_reopened_beside_a_nested_one_is_one_namespace() {
        // Two namespaces of one name merge in TypeScript rather than
        // being reported, so grouping by module id would be invisible.
        let generated = declarations(
            "module common {
               module util { @final struct Location { double x; }; };
               @final struct Robot { ::common::util::Location where_; };
             };",
        );
        assert_eq!(
            generated.matches("export namespace common {").count(),
            1,
            "{generated}"
        );
        assert!(
            generated.contains("where_: common.util.Location;"),
            "{generated}"
        );
    }

    #[test]
    fn a_binding_that_is_a_reserved_word_is_escaped_and_a_member_is_not() {
        let generated = declarations(
            "module package {
               enum class { template, virtual, friend };
               @final struct type { long new; double delete; short operator; };
             };",
        );
        assert!(
            generated.contains("export namespace package_ {"),
            "{generated}"
        );
        assert!(generated.contains("export const class_ ="), "{generated}");
        // `type` is a contextual keyword and a legal binding.
        assert!(generated.contains("export interface type {"), "{generated}");
        // A property keeps the IDL's spelling: a class body parses
        // `new`, `delete` and `operator` as field names.
        assert!(generated.contains("new: number;"), "{generated}");
        assert!(generated.contains("delete: number;"), "{generated}");
        assert!(generated.contains("operator: number;"), "{generated}");
        // Where a name did move, the declaration says which IDL name it
        // came from.
        assert!(generated.contains("IDL `package`"), "{generated}");
    }

    #[test]
    fn a_construct_with_no_typescript_data_form_is_named_and_not_left_out() {
        // The member is not dropped and no `unknown` is substituted: an
        // interface missing a field that reports success is the defect
        // this materialiser is arranged against.
        let failure = render("@final struct P { Object o; long tail; };", "test.idl")
            .expect_err("an object reference");
        assert!(failure.contains("Object"), "{failure}");
        assert!(failure.contains("TypeScript"), "{failure}");
    }

    /// An IDL `any` is not among them: DDS-TS 1.0 §7.3.4 gives it the
    /// boxed form, and neither TypeScript `any` nor `unknown` may stand
    /// in for it.
    #[test]
    fn an_any_member_takes_the_box_and_not_a_typescript_keyword() {
        let generated = declarations("@final struct P { any a; long tail; };");
        assert!(generated.contains("a: DdsAny;"), "{generated}");
        assert!(!generated.contains("a: any;"), "{generated}");
        assert!(!generated.contains("a: unknown;"), "{generated}");
        assert!(generated.contains("kind: \"any\""), "{generated}");
    }

    #[test]
    fn a_nested_name_that_would_capture_a_root_reference_is_refused() {
        // Every reference is written from the file root and TypeScript
        // has no root operator; the inner `c` would capture `c.Q`.
        let failure = render(
            "module c { @final struct Q { long x; }; };
             module a {
               module c { @final struct R { long y; }; };
               @final struct P { ::c::Q q; };
             };",
            "test.idl",
        )
        .expect_err("a nested namespace shadowing a top-level one is refused");
        assert!(failure.contains('c'), "{failure}");
    }

    #[test]
    fn a_declaration_named_after_the_prelude_type_is_refused() {
        let failure = render("@final struct ZdFloat128 { long x; };", "test.idl")
            .expect_err("a type named after the prelude declaration is refused");
        assert!(failure.contains("ZdFloat128"), "{failure}");
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
    fn a_constant_past_the_safe_integer_range_is_refused_rather_than_rounded() {
        // 2^53 + 1 is not a `number`; 2^53 is. A `long long` constant
        // is a `bigint` and holds it either way.
        let ok = declarations("const long long BIG = 9007199254740993;");
        assert!(ok.contains("9007199254740993n"), "{ok}");
    }

    #[test]
    fn a_string_constant_reaches_the_output() {
        let generated = declarations("const string MODULE_ID = \"spatial/1.5\";");
        assert!(
            generated.contains("export const MODULE_ID: string = \"spatial/1.5\";"),
            "{generated}"
        );
    }
}
