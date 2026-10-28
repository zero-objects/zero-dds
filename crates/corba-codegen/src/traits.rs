// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Annex A.1.1 type traits — the per-type table a CORBA PSM declares
//! beside a generated type.
//!
//! IDL4 to C++ 1.0 Annex A.1.1, pp. 33–37: "For any IDL defined type a
//! CORBA type trait shall be provided to facilitate template meta
//! programming. The generic type trait for IDL type T shall be available
//! as `CORBA::traits<T>`." Table A.1 lists the four members every type
//! has; Tables A.2 to A.12 add the members a particular kind of type
//! has. This module answers, for a type in the model, which members it
//! gets and what each one is defined as — and renders that in the four
//! PSM columns.
//!
//! ## What the tables actually require
//!
//! | table | applies to | members |
//! |---|---|---|
//! | A.1 | any IDL type | `value_type`, `in_type`, `out_type`, `inout_type` |
//! | A.2 | interfaces | `is_local`, `is_abstract`, `ref_type`, `weak_ref_type` |
//! | A.3 / A.4 | strings | `element_traits`, `is_bounded`, and `bound` when bounded |
//! | A.5 / A.6 | sequences | `element_traits`, `is_bounded`, and `bound` when bounded |
//! | A.7 | arrays | `element_traits`, `dimensions` |
//! | A.8 | valuetypes | `is_abstract`, `is_truncatable`, `factory_type`, `obv_type` |
//! | A.9 | valueboxes | `boxed_traits` |
//! | A.10 / A.11 | maps | `key_traits`, `value_traits`, `is_bounded`, and `bound` |
//! | A.12 | bitmasks | `bit_bound`, `underlying_type` |
//!
//! The predecessor of this module put `is_local` on every type. Table
//! A.2 gives it to interfaces and to nothing else, and a `struct` whose
//! trait declares `is_local` declares a member the specification does
//! not have. That is why [`traits_of`] is written from the tables and
//! not from a fixed member list.
//!
//! ## Where the passing forms come from, and where they do not
//!
//! Table A.13 groups "Interfaces, Valuetypes, `CORBA::TypeCode`, Enums,
//! and Basic Types" and gives them `CORBA::traits<T>::ref_type`; Table
//! A.14 groups structured types, sequences and strings and gives them
//! `const T&`, `T&`, `T&` and `T`.
//!
//! A.13's grouping cannot be taken literally for enums and basic types:
//! `ref_type` is a member Table A.2 gives to **interfaces**, so
//! `CORBA::traits<int32_t>::ref_type` names a member that no table
//! declares. This module therefore follows A.13 for the three reference
//! kinds it can be about — an object reference, a valuetype and
//! `TypeCode` — and passes an enum or a basic type by value, which is
//! what every C++ mapping in print does with a `long`.
//!
//! These are **not** the forms [`crate::stub::cpp_param_type`] answers.
//! That function implements the classic IDL to C++ mapping,
//! `formal/2008-01-09` §1.22, and its answers are checked against what
//! `omniidl`, `tao_idl` and JacORB's `idl` actually generate
//! (`tests/orb-corpus/recorded/`). Two documents, two answers, and the
//! difference is the point of the traits layer: a generated header that
//! declares `CORBA::traits<T>::in_type` lets a caller write one template
//! against both.
//!
//! ## The three columns that have no templates
//!
//! Java, C# and Rust have no compile-time type traits in the C++ sense,
//! and the OMG published no traits table for any of them. Their
//! renderers write the **same table** as a companion declaration of
//! constants — `SCREAMING_SNAKE` in Java, `PascalCase` in C#, a module
//! of `const` in Rust — so a generator, an ORB or a test can read what
//! the C++ column states in types. Where a member is a type rather than
//! a value it is carried as its name, and where it is a flag or a count
//! it keeps its natural type. This is this project's transliteration and
//! is documented as such, in the same way the crate's Rust column of the
//! special-type table is.
//!
//! An **alias has no traits of its own**. [`traits_of`] answers `None`
//! for a `typedef`: C++ has typedefs, so `CORBA::traits<probe::Money>`
//! and `CORBA::traits<CORBA::Fixed>` are one specialisation, and
//! emitting both is a redefinition the compiler rejects. A caller asks
//! for the aliased type instead, which is where Tables A.3 to A.11 live
//! anyway — a `sequence`, a `string` and a `map` are usually written as
//! the target of a `typedef`.

use alloc::borrow::ToOwned;
use alloc::boxed::Box as AllocBox;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use zerodds_ast::{Ast, ParamDirection, TypeKind, TypeNode, ValuetypeKind};
use zerodds_ast_core::Id;
use zerodds_lang_cst::{Doc, Ident, Literal, Path, Visibility, cpp, csharp, java, rust};

use crate::special_types::{SpecialType, TargetLanguage};
use crate::stub::{
    NoRenderer, StubType, TypeShape, csharp_return_type, java_holder_type, java_return_type,
    stub_type_of,
};

/// How deep the element walk of a multidimensional array follows.
///
/// Table A.7: "The element_traits for a multidimensional array shall be
/// the type trait for the core type." An array of arrays is the shape
/// that needs the walk, and IDL bounds the nesting by the declaration
/// itself; this is here so a model built by hand with a cycle stops.
const MAX_ELEMENT_DEPTH: u32 = 64;

/// One member of a type's trait, as Annex A.1.1's tables name it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitMember {
    /// The member's name, spelled as the tables spell it —
    /// `value_type`, `is_bounded`, `bit_bound`. Each column's renderer
    /// converts it to its own convention.
    pub name: &'static str,
    /// What the member is defined as.
    pub definition: TraitDefinition,
}

/// What a trait member is defined as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraitDefinition {
    /// The traited type itself. Table A.1 `value_type`.
    SelfType,
    /// The type to be used for a parameter of this direction. Table
    /// A.1, resolved through Tables A.13 and A.14.
    Passing(ParamDirection),
    /// A compile-time boolean: `std::true_type` or `std::false_type` in
    /// C++, a `bool` constant in the other three.
    Flag(bool),
    /// A compile-time `uint32_t`: `std::integral_constant` in C++, an
    /// integer constant in the other three.
    Constant(u32),
    /// `CORBA::traits<X>` for another type X, named as the IDL writes
    /// it: a scoped name, or `char` / `wchar_t` for a string's element.
    TraitsOf(String),
    /// A type Annex A.1.1 derives from the traited one.
    Derived(DerivedForm),
}

/// A trait member whose definition is derived from the traited type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DerivedForm {
    /// `ref_type` — the strong reference. Table A.2.
    Ref,
    /// `weak_ref_type` — the weak reference. Table A.2.
    WeakRef,
    /// `factory_type` — `CORBA::traits<T>::ref_type`. Table A.8.
    Factory,
    /// `obv_type` — the OBV class carrying the default accessor and
    /// modifier implementations. Table A.8; the class is
    /// `OBV_<scoped name>` in the classic C++ mapping, which is where
    /// the name comes from.
    Obv,
    /// `underlying_type` — the integer a bitmask is held in, wide
    /// enough for its bit bound. Table A.12.
    Underlying(u16),
}

/// The Annex A.1.1 trait of one type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeTraits {
    /// The type's scoped name in the source's `::` form, or the empty
    /// string for an anonymous type — an inline `sequence<long>` has a
    /// trait and no name of its own.
    pub scoped_name: String,
    /// What the passing forms are computed from. The same value
    /// [`crate::stub::stub_type_of`] gives, so the traits layer and the
    /// stub layer cannot disagree about what a type is.
    pub subject: StubType,
    /// The members, in the order the tables list them: Table A.1's four
    /// first, then the kind-specific ones.
    pub members: Vec<TraitMember>,
}

impl TypeTraits {
    /// The member with this name, if the type has one.
    #[must_use]
    pub fn member(&self, name: &str) -> Option<&TraitMember> {
        self.members.iter().find(|member| member.name == name)
    }

    /// The names of every member, in table order.
    #[must_use]
    pub fn member_names(&self) -> Vec<&'static str> {
        self.members.iter().map(|member| member.name).collect()
    }

    /// The name a companion declaration takes in the three columns that
    /// have no templates: the type's own name with `CorbaTraits` after
    /// it, scope segments dropped.
    ///
    /// The scope is dropped because the companion is generated **into**
    /// the type's own scope — a Java package, a C# namespace, a Rust
    /// module — so `A::B::S` is `SCorbaTraits` beside `S` and not
    /// `A_B_SCorbaTraits` somewhere else. An anonymous type has no
    /// companion name and answers `None`.
    #[must_use]
    pub fn companion_name(&self) -> Option<String> {
        let last = self.scoped_name.rsplit("::").next()?;
        if last.is_empty() {
            return None;
        }
        Some(format!("{last}CorbaTraits"))
    }
}

/// The Annex A.1.1 trait of a type in the model.
///
/// `None` for a `typedef` and for a type node that is not in the model,
/// for the reason the module documentation gives: an alias is not a
/// distinct C++ type, so a specialisation on it is a redefinition of the
/// aliased type's.
#[must_use]
pub fn traits_of(ast: &Ast, id: Id<TypeNode>) -> Option<TypeTraits> {
    let node = ast.type_(id).ok()?;
    if matches!(node.kind, TypeKind::Alias { .. }) {
        return None;
    }
    let subject = stub_type_of(ast, id);
    let mut members = common_members();
    members.extend(kind_members(ast, id, &node.kind));
    Some(TypeTraits {
        scoped_name: scoped_name_of(ast, id),
        subject,
        members,
    })
}

/// Table A.1 — the four members every IDL type has.
fn common_members() -> Vec<TraitMember> {
    alloc::vec![
        TraitMember {
            name: "value_type",
            definition: TraitDefinition::SelfType,
        },
        TraitMember {
            name: "in_type",
            definition: TraitDefinition::Passing(ParamDirection::In),
        },
        TraitMember {
            name: "out_type",
            definition: TraitDefinition::Passing(ParamDirection::Out),
        },
        TraitMember {
            name: "inout_type",
            definition: TraitDefinition::Passing(ParamDirection::InOut),
        },
    ]
}

/// Tables A.2 to A.12 — the members a kind of type adds.
fn kind_members(ast: &Ast, id: Id<TypeNode>, kind: &TypeKind) -> Vec<TraitMember> {
    match kind {
        // A.3 / A.4 — strings. The element is `char` or `wchar_t`, which
        // Table A.3 names outright, so it is written and not looked up.
        TypeKind::String { width, bound } => {
            let element = match width {
                zerodds_ast::CharWidth::Char => "char",
                zerodds_ast::CharWidth::WChar => "wchar_t",
            };
            let mut out = alloc::vec![TraitMember {
                name: "element_traits",
                definition: TraitDefinition::TraitsOf(element.to_owned()),
            }];
            out.extend(bounded_members(ast, *bound));
            out
        }
        // A.5 / A.6 — sequences.
        TypeKind::Sequence { element, bound } => {
            let mut out = alloc::vec![TraitMember {
                name: "element_traits",
                definition: TraitDefinition::TraitsOf(
                    element
                        .resolved()
                        .map_or_else(String::new, |target| scoped_name_of(ast, target)),
                ),
            }];
            out.extend(bounded_members(ast, *bound));
            out
        }
        // A.7 — arrays. `element_traits` is the **core** type's, so an
        // array of arrays walks through to the innermost element.
        TypeKind::Array { element, dims } => {
            let core = element
                .resolved()
                .map_or_else(String::new, |target| core_element_name(ast, target, 0));
            alloc::vec![
                TraitMember {
                    name: "element_traits",
                    definition: TraitDefinition::TraitsOf(core),
                },
                TraitMember {
                    name: "dimensions",
                    definition: TraitDefinition::Constant(
                        u32::try_from(dims.len()).unwrap_or(u32::MAX),
                    ),
                },
            ]
        }
        // A.10 / A.11 — maps.
        TypeKind::Map { key, value, bound } => {
            let mut out = alloc::vec![
                TraitMember {
                    name: "key_traits",
                    definition: TraitDefinition::TraitsOf(
                        key.resolved()
                            .map_or_else(String::new, |target| scoped_name_of(ast, target)),
                    ),
                },
                TraitMember {
                    name: "value_traits",
                    definition: TraitDefinition::TraitsOf(
                        value
                            .resolved()
                            .map_or_else(String::new, |target| scoped_name_of(ast, target)),
                    ),
                },
            ];
            out.extend(bounded_members(ast, *bound));
            out
        }
        // A.8 — valuetypes.
        TypeKind::Valuetype { kind, .. } => alloc::vec![
            TraitMember {
                name: "is_abstract",
                definition: TraitDefinition::Flag(matches!(kind, ValuetypeKind::Abstract)),
            },
            TraitMember {
                name: "is_truncatable",
                definition: TraitDefinition::Flag(matches!(kind, ValuetypeKind::Truncatable)),
            },
            TraitMember {
                name: "factory_type",
                definition: TraitDefinition::Derived(DerivedForm::Factory),
            },
            TraitMember {
                name: "obv_type",
                definition: TraitDefinition::Derived(DerivedForm::Obv),
            },
        ],
        // A.9 — valueboxes.
        TypeKind::ValueBox { boxed } => alloc::vec![TraitMember {
            name: "boxed_traits",
            definition: TraitDefinition::TraitsOf(
                boxed
                    .resolved()
                    .map_or_else(String::new, |target| scoped_name_of(ast, target)),
            ),
        }],
        // A.12 — bitmasks. The bit bound is the model's effective one,
        // written or defaulted, so a `@bit_bound(9)` and an unannotated
        // bitmask do not both answer 32.
        TypeKind::Bitmask { .. } => {
            let bits = ast.effective_bit_bound(id);
            alloc::vec![
                TraitMember {
                    name: "bit_bound",
                    definition: TraitDefinition::Constant(u32::from(bits)),
                },
                TraitMember {
                    name: "underlying_type",
                    definition: TraitDefinition::Derived(DerivedForm::Underlying(bits)),
                },
            ]
        }
        // A.2 — interfaces, reached through an object reference. The
        // generic `Object` has no declaration to read the two flags
        // from, and both are false for it: it is neither local nor
        // abstract.
        TypeKind::ObjectRef { interface } => {
            let declared = interface
                .as_ref()
                .and_then(zerodds_ast_core::Ref::resolved)
                .and_then(|id| ast.interface(id).ok());
            alloc::vec![
                TraitMember {
                    name: "is_local",
                    definition: TraitDefinition::Flag(declared.is_some_and(|node| {
                        matches!(node.kind, zerodds_ast::InterfaceKind::Local)
                    }),),
                },
                TraitMember {
                    name: "is_abstract",
                    definition: TraitDefinition::Flag(declared.is_some_and(|node| {
                        matches!(node.kind, zerodds_ast::InterfaceKind::Abstract)
                    }),),
                },
                TraitMember {
                    name: "ref_type",
                    definition: TraitDefinition::Derived(DerivedForm::Ref),
                },
                TraitMember {
                    name: "weak_ref_type",
                    definition: TraitDefinition::Derived(DerivedForm::WeakRef),
                },
            ]
        }
        // Table A.1 and nothing else: a structure, a union, an
        // enumeration, a bitset, a `fixed`, a primitive, an `any`, a
        // `native`, `ValueBase` and an annotation type.
        TypeKind::Structure { .. }
        | TypeKind::Union { .. }
        | TypeKind::Enumeration { .. }
        | TypeKind::Bitset { .. }
        | TypeKind::Fixed { .. }
        | TypeKind::Primitive(_)
        | TypeKind::Any
        | TypeKind::Native
        | TypeKind::ValueBase
        | TypeKind::AnnotationType { .. }
        | TypeKind::Alias { .. } => Vec::new(),
    }
}

/// `is_bounded`, and `bound` where the declaration carries one.
///
/// Tables A.3/A.4, A.5/A.6 and A.10/A.11 are the same pair of tables
/// three times: the unbounded form declares `is_bounded` alone and the
/// bounded one adds `bound`.
fn bounded_members(ast: &Ast, bound: zerodds_ast::Bound) -> Vec<TraitMember> {
    let Some(expr) = bound.expr() else {
        return alloc::vec![TraitMember {
            name: "is_bounded",
            definition: TraitDefinition::Flag(false),
        }];
    };
    let value = ast
        .eval_integral(expr)
        .ok()
        .and_then(|value| u32::try_from(value).ok());
    let mut out = alloc::vec![TraitMember {
        name: "is_bounded",
        definition: TraitDefinition::Flag(true),
    }];
    // A bound that does not evaluate leaves `is_bounded` true and omits
    // `bound`: the declaration is bounded either way, and a `bound` of
    // an invented value would be worse than an absent one.
    if let Some(value) = value {
        out.push(TraitMember {
            name: "bound",
            definition: TraitDefinition::Constant(value),
        });
    }
    out
}

/// The core element of an array, following nested arrays. Table A.7.
///
/// zerodds-lint: recursion-depth 64 (array-of-array nesting; bounded by
/// `MAX_ELEMENT_DEPTH`, past which the walk stops and names the type it
/// stood on rather than descending further). An IDL declaration bounds
/// the nesting by itself; the cap is what a hand-built model with a
/// cycle in its element chain runs into.
fn core_element_name(ast: &Ast, id: Id<TypeNode>, depth: u32) -> String {
    if depth >= MAX_ELEMENT_DEPTH {
        return scoped_name_of(ast, id);
    }
    if let Ok(node) = ast.type_(id)
        && let TypeKind::Array { element, .. } = &node.kind
        && let Some(inner) = element.resolved()
    {
        return core_element_name(ast, inner, depth + 1);
    }
    scoped_name_of(ast, id)
}

/// The scoped name of a type node, in the source's `::` form, or the
/// empty string where the type has none.
fn scoped_name_of(ast: &Ast, id: Id<TypeNode>) -> String {
    let Ok(node) = ast.type_(id) else {
        return String::new();
    };
    ast.scoped_name_of(zerodds_ast::AnyId::Type(id, node.node_id))
        .map_or_else(|| node.name.clone(), |scoped| scoped.segments().join("::"))
}

/// Render a type's trait declaration as text, in one column.
///
/// # Errors
/// [`NoRenderer`] for a column with an OMG mapping and no CST, of which
/// there is none — see [`crate::stub::render_stub_op`].
pub fn render_type_traits(
    traits: &TypeTraits,
    language: TargetLanguage,
) -> Result<String, NoRenderer> {
    match language {
        TargetLanguage::Cpp => Ok(zerodds_lang_render::cpp::decl(&cpp_traits_specialisation(
            traits,
        ))),
        TargetLanguage::Java => Ok(zerodds_lang_render::java::type_decl(&java_traits_class(
            traits,
        ))),
        TargetLanguage::CSharp => Ok(zerodds_lang_render::csharp::type_decl(
            &csharp_traits_class(traits),
        )),
        TargetLanguage::Rust => Ok(zerodds_lang_render::rust::item(&rust::Item::Module(
            rust_traits_module(traits),
        ))),
    }
}

// ---------------------------------------------------------------------
// C++ — the normative column. IDL4 to C++ 1.0 Annex A.1.1.
// ---------------------------------------------------------------------

/// `template <> struct traits<T> { ... };` — Annex A.1.1's own form.
///
/// The declaration goes inside `namespace CORBA`; this builds the
/// specialisation and leaves the namespace to the caller, which is where
/// the rest of a generated header's structure is decided.
#[must_use]
pub fn cpp_traits_specialisation(traits: &TypeTraits) -> cpp::Decl {
    let subject = cpp_subject(traits);
    let members: Vec<cpp::ClassMember> = cpp_declaration_order(traits)
        .into_iter()
        .map(|member| {
            cpp::ClassMember::Alias(cpp::Alias {
                doc: Doc::none(),
                name: Ident::new(member.name),
                type_: cpp_definition(traits, &subject, &member.definition),
            })
        })
        .collect();
    cpp::Decl::Template(cpp::Template {
        doc: Doc::line(&format!(
            "Annex A.1.1 type traits for `{}`.",
            traits.display_name()
        )),
        params: Vec::new(),
        decl: AllocBox::new(cpp::Decl::Class(cpp::Class {
            doc: Doc::none(),
            is_struct: true,
            name: Ident::new("traits"),
            bases: Vec::new(),
            sections: alloc::vec![cpp::Section {
                access: cpp::Access::Public,
                members,
            }],
        })),
        specialisation: alloc::vec![subject.clone()],
    })
}

/// The members in an order C++ accepts.
///
/// [`TypeTraits::members`] is in the tables' order, which is what a
/// reader compares against Annex A.1.1 — and which C++ rejects for a
/// reference kind: Table A.1 lists `in_type` before Table A.2 lists
/// `ref_type`, and a member alias must be declared before the alias that
/// names it. So `ref_type` and `weak_ref_type` are hoisted to just after
/// `value_type`. Nothing else in the tables refers to another member,
/// which is why this is a hoist of two names rather than a sort.
fn cpp_declaration_order(traits: &TypeTraits) -> Vec<&TraitMember> {
    let hoisted = ["ref_type", "weak_ref_type"];
    let mut out: Vec<&TraitMember> = traits
        .members
        .iter()
        .filter(|member| member.name == "value_type")
        .collect();
    out.extend(
        traits
            .members
            .iter()
            .filter(|member| hoisted.contains(&member.name)),
    );
    out.extend(
        traits
            .members
            .iter()
            .filter(|member| member.name != "value_type" && !hoisted.contains(&member.name)),
    );
    out
}

/// The C++ type the trait is specialised on.
fn cpp_subject(traits: &TypeTraits) -> cpp::Type {
    match &traits.subject {
        StubType::Special(t) => crate::special_types::cpp_type(*t),
        StubType::Void => cpp::Type::Void,
        StubType::Other { .. } => cpp::Type::named(cpp_path(&traits.scoped_name)),
    }
}

fn cpp_path(scoped: &str) -> Path {
    let segments: Vec<&str> = scoped.split("::").filter(|s| !s.is_empty()).collect();
    Path::rooted(&segments)
}

/// One member's C++ definition.
fn cpp_definition(
    traits: &TypeTraits,
    subject: &cpp::Type,
    definition: &TraitDefinition,
) -> cpp::Type {
    match definition {
        TraitDefinition::SelfType => subject.clone(),
        TraitDefinition::Passing(direction) => cpp_passing(traits, subject, *direction),
        TraitDefinition::Flag(value) => cpp::Type::named(Path::new(&[
            "std",
            if *value { "true_type" } else { "false_type" },
        ])),
        TraitDefinition::Constant(value) => cpp::Type::template(
            Path::new(&["std", "integral_constant"]),
            alloc::vec![
                cpp::Type::Primitive(cpp::Primitive::UInt32),
                cpp::Type::named(Path::single(&value.to_string())),
            ],
        ),
        TraitDefinition::TraitsOf(name) => cpp::Type::template(
            Path::new(&["CORBA", "traits"]),
            alloc::vec![cpp_named_or_keyword(name)],
        ),
        TraitDefinition::Derived(form) => cpp_derived(traits, subject, *form),
    }
}

/// A `char` or `wchar_t` stays a keyword; anything else is a scoped name.
fn cpp_named_or_keyword(name: &str) -> cpp::Type {
    match name {
        "char" => cpp::Type::Primitive(cpp::Primitive::Char),
        "wchar_t" => cpp::Type::Primitive(cpp::Primitive::WChar),
        "" => cpp::Type::Void,
        other => cpp::Type::named(cpp_path(other)),
    }
}

/// Tables A.13 and A.14 — the passing form of a direction.
fn cpp_passing(traits: &TypeTraits, subject: &cpp::Type, direction: ParamDirection) -> cpp::Type {
    if is_reference_kind(&traits.subject) {
        // Table A.13. `ref_type` is a member of this very trait, so it
        // is named through the specialisation rather than repeated.
        let ref_type = cpp::Type::named(Path::new(&["ref_type"]));
        return match direction {
            ParamDirection::In => ref_type,
            ParamDirection::Out | ParamDirection::InOut => cpp::Type::Ref(AllocBox::new(ref_type)),
        };
    }
    // Table A.14.
    match direction {
        ParamDirection::In => cpp::Type::Ref(AllocBox::new(cpp::Type::Const(AllocBox::new(
            subject.clone(),
        )))),
        ParamDirection::Out | ParamDirection::InOut => {
            cpp::Type::Ref(AllocBox::new(subject.clone()))
        }
    }
}

/// Whether Table A.13's reference row is the one that applies.
///
/// An object reference, a valuetype and `CORBA::TypeCode`. Not an enum
/// and not a basic type: `ref_type` is Table A.2's member and neither of
/// those has one, which the module documentation states at length.
fn is_reference_kind(subject: &StubType) -> bool {
    if matches!(
        subject,
        StubType::Special(SpecialType::TypeCode | SpecialType::Object | SpecialType::ValueBase)
    ) {
        return true;
    }
    matches!(
        subject.shape(),
        TypeShape::ObjectReference | TypeShape::Valuetype
    )
}

fn cpp_derived(traits: &TypeTraits, subject: &cpp::Type, form: DerivedForm) -> cpp::Type {
    match form {
        DerivedForm::Ref => cpp::Type::template(
            Path::new(&["omg", "types", "ref_type"]),
            alloc::vec![subject.clone()],
        ),
        DerivedForm::WeakRef => cpp::Type::template(
            Path::new(&["omg", "types", "weak_ref_type"]),
            alloc::vec![subject.clone()],
        ),
        // Table A.8: "The `CORBA::traits<T>::ref_type` of the
        // valuetype." Written out as the reference form rather than as
        // `traits<T>::ref_type`, because Table A.2 gives `ref_type` to
        // interfaces and Table A.8's valuetype set does not list one —
        // a self-reference to a member the specialisation does not
        // declare would not compile.
        DerivedForm::Factory => cpp::Type::template(
            Path::new(&["omg", "types", "ref_type"]),
            alloc::vec![subject.clone()],
        ),
        DerivedForm::Obv => cpp::Type::named(cpp_path(&obv_name(&traits.scoped_name))),
        DerivedForm::Underlying(bits) => cpp::Type::Primitive(underlying_primitive(bits)),
    }
}

/// `OBV_<scoped name>`, the classic C++ mapping's name for the class
/// that carries a valuetype's default accessor implementations.
///
/// The prefix goes on the innermost segment: `A::B::V` is `A::B::OBV_V`,
/// because the class is generated in the valuetype's own scope.
fn obv_name(scoped: &str) -> String {
    let mut segments: Vec<&str> = scoped.split("::").filter(|s| !s.is_empty()).collect();
    let Some(last) = segments.pop() else {
        return "OBV_".to_string();
    };
    let prefixed = format!("OBV_{last}");
    if segments.is_empty() {
        return prefixed;
    }
    format!("{}::{prefixed}", segments.join("::"))
}

/// The unsigned integer a bit bound is held in. Table A.12, and the
/// same widths §7.17.4's `@bit_bound` table gives an enumeration.
const fn underlying_primitive(bits: u16) -> cpp::Primitive {
    match bits {
        0..=8 => cpp::Primitive::UInt8,
        9..=16 => cpp::Primitive::UInt16,
        17..=32 => cpp::Primitive::UInt32,
        _ => cpp::Primitive::UInt64,
    }
}

// ---------------------------------------------------------------------
// The three columns with no templates.
// ---------------------------------------------------------------------

impl TypeTraits {
    /// The name to write in documentation: the scoped name, or the IDL
    /// spelling of a special type where the node is anonymous.
    fn display_name(&self) -> String {
        if !self.scoped_name.is_empty() {
            return self.scoped_name.clone();
        }
        match &self.subject {
            StubType::Special(t) => t.idl_spelling().to_string(),
            _ => "<anonymous>".to_string(),
        }
    }
}

/// The Java companion class: `<Name>CorbaTraits`.
///
/// A `final` class of `public static final` constants and a private
/// constructor, carrying the same table the C++ specialisation states in
/// types. Java has no compile-time type traits, so a member that is a
/// type is carried as its name.
#[must_use]
pub fn java_traits_class(traits: &TypeTraits) -> java::TypeDecl {
    let name = traits
        .companion_name()
        .unwrap_or_else(|| "CorbaTraits".to_string());
    let mut members: Vec<java::Member> = traits
        .members
        .iter()
        .map(|member| {
            let (type_, value) = java_constant(traits, &member.definition);
            java::Member::Field(java::Field {
                doc: Doc::line(&format!("Annex A.1.1 `{}`.", member.name)),
                annotations: Vec::new(),
                modifiers: alloc::vec![
                    java::Modifier::Public,
                    java::Modifier::Static,
                    java::Modifier::Final,
                ],
                type_,
                name: Ident::new(&screaming_snake(member.name)),
                value: Some(value),
            })
        })
        .collect();
    members.push(java::Member::Method(java::Method {
        doc: Doc::line("Not instantiable: the class is a table."),
        annotations: Vec::new(),
        modifiers: alloc::vec![java::Modifier::Private],
        generics: Vec::new(),
        returns: java::Type::Void,
        name: Ident::new(&name),
        params: Vec::new(),
        throws: Vec::new(),
        body: Some(java::Block {
            statements: Vec::new(),
        }),
    }));
    java::TypeDecl::Class(java::Class {
        doc: Doc::line(&format!(
            "Annex A.1.1 type traits for `{}`.",
            traits.display_name()
        )),
        annotations: Vec::new(),
        modifiers: alloc::vec![java::Modifier::Public, java::Modifier::Final],
        name: Ident::new(&name),
        generics: Vec::new(),
        extends: None,
        implements: Vec::new(),
        members,
    })
}

fn java_constant(traits: &TypeTraits, definition: &TraitDefinition) -> (java::Type, java::Expr) {
    match definition {
        TraitDefinition::Flag(value) => (
            java::Type::Primitive(java::Primitive::Boolean),
            java::Expr::Literal(Literal::Bool(*value)),
        ),
        TraitDefinition::Constant(value) => (
            java::Type::Primitive(java::Primitive::Int),
            java::Expr::Literal(Literal::int(i128::from(*value))),
        ),
        other => (
            java::Type::named(Path::new(&["java", "lang", "String"])),
            java::Expr::Literal(Literal::string(&java_name(traits, other))),
        ),
    }
}

/// The name a Java trait member carries.
///
/// The passing rows are the ones with real content: `in_type` is the
/// mapped value type and `out_type` and `inout_type` are the **holder**
/// class IDL to Java §1.4 routes them through, which
/// [`crate::stub::java_holder_type`] already answers.
fn java_name(traits: &TypeTraits, definition: &TraitDefinition) -> String {
    match definition {
        TraitDefinition::SelfType => {
            zerodds_lang_render::java::type_(&java_return_type(&traits.subject))
        }
        TraitDefinition::Passing(ParamDirection::In) => {
            zerodds_lang_render::java::type_(&java_return_type(&traits.subject))
        }
        TraitDefinition::Passing(_) => {
            zerodds_lang_render::java::type_(&java_holder_type(&traits.subject))
        }
        TraitDefinition::TraitsOf(name) => name.clone(),
        TraitDefinition::Derived(form) => derived_idl_name(traits, *form),
        // Answered by `java_constant` before this is reached; kept
        // exhaustive so a new definition kind is a compile error.
        TraitDefinition::Flag(value) => value.to_string(),
        TraitDefinition::Constant(value) => value.to_string(),
    }
}

/// The C# companion class: `<Name>CorbaTraits`.
///
/// A `static class` of `public const`, in .NET's naming convention. The
/// same table as the Java column and the same reason for it.
#[must_use]
pub fn csharp_traits_class(traits: &TypeTraits) -> csharp::TypeDecl {
    let name = traits
        .companion_name()
        .unwrap_or_else(|| "CorbaTraits".to_string());
    let members: Vec<csharp::Member> = traits
        .members
        .iter()
        .map(|member| {
            let (type_, value) = csharp_constant(traits, &member.definition);
            csharp::Member::Field(csharp::Field {
                doc: Doc::line(&format!("Annex A.1.1 `{}`.", member.name)),
                attributes: Vec::new(),
                modifiers: alloc::vec![csharp::Modifier::Public, csharp::Modifier::Const],
                type_,
                name: Ident::new(&pascal(member.name)),
                value: Some(value),
            })
        })
        .collect();
    csharp::TypeDecl::Class(csharp::Class {
        doc: Doc::line(&format!(
            "Annex A.1.1 type traits for `{}`.",
            traits.display_name()
        )),
        attributes: Vec::new(),
        modifiers: alloc::vec![csharp::Modifier::Public, csharp::Modifier::Static],
        name: Ident::new(&name),
        generics: Vec::new(),
        bases: Vec::new(),
        constraints: Vec::new(),
        members,
    })
}

fn csharp_constant(
    traits: &TypeTraits,
    definition: &TraitDefinition,
) -> (csharp::Type, csharp::Expr) {
    match definition {
        TraitDefinition::Flag(value) => (
            csharp::Type::Predefined(csharp::Predefined::Bool),
            csharp::Expr::Literal(Literal::Bool(*value)),
        ),
        TraitDefinition::Constant(value) => (
            csharp::Type::Predefined(csharp::Predefined::UInt),
            csharp::Expr::Literal(Literal::int(i128::from(*value))),
        ),
        other => (
            csharp::Type::Predefined(csharp::Predefined::String),
            csharp::Expr::Literal(Literal::string(&csharp_name(traits, other))),
        ),
    }
}

/// The name a C# trait member carries.
///
/// `in_type`, `out_type` and `inout_type` are all the **same** type: C#
/// carries the direction on the parameter modifier, which
/// [`crate::stub::csharp_param_modifier`] answers, so the three rows of
/// Table A.1 differ only in the modifier they imply. That is stated here
/// rather than left for a reader to infer from three identical strings.
fn csharp_name(traits: &TypeTraits, definition: &TraitDefinition) -> String {
    match definition {
        TraitDefinition::SelfType | TraitDefinition::Passing(_) => {
            zerodds_lang_render::csharp::type_(&csharp_return_type(&traits.subject))
        }
        TraitDefinition::TraitsOf(name) => name.clone(),
        TraitDefinition::Derived(form) => derived_idl_name(traits, *form),
        TraitDefinition::Flag(value) => value.to_string(),
        TraitDefinition::Constant(value) => value.to_string(),
    }
}

/// The Rust companion module: `mod <name>_corba_traits`.
///
/// Rust has associated types and could carry this on a trait, and does
/// not: the `CorbaTraits` trait would have to be declared somewhere, and
/// the ORB crate that would declare it is among the packages this
/// rebuild has not reached. A module of `const` names nothing that does
/// not resolve — the same standard the crate's stub column holds itself
/// to for a CORBA exception type.
#[must_use]
pub fn rust_traits_module(traits: &TypeTraits) -> rust::Module {
    let name = traits
        .companion_name()
        .map(|companion| snake(&companion))
        .unwrap_or_else(|| "corba_traits".to_string());
    let items: Vec<rust::Item> = traits
        .members
        .iter()
        .map(|member| {
            let (type_, value) = rust_constant(traits, &member.definition);
            rust::Item::Const(rust::Const {
                doc: Doc::line(&format!("Annex A.1.1 `{}`.", member.name)),
                attrs: Vec::new(),
                visibility: Visibility::Public,
                name: Ident::new(&screaming_snake(member.name)),
                type_,
                value,
            })
        })
        .collect();
    rust::Module {
        doc: Doc::line(&format!(
            "Annex A.1.1 type traits for `{}`.",
            traits.display_name()
        )),
        attrs: Vec::new(),
        visibility: Visibility::Public,
        name: Ident::new(&name),
        uses: Vec::new(),
        items,
    }
}

fn rust_constant(traits: &TypeTraits, definition: &TraitDefinition) -> (rust::Type, rust::Expr) {
    match definition {
        TraitDefinition::Flag(value) => (
            rust::Type::Primitive(rust::Primitive::Bool),
            rust::Expr::Literal(Literal::Bool(*value)),
        ),
        TraitDefinition::Constant(value) => (
            rust::Type::Primitive(rust::Primitive::U32),
            rust::Expr::Literal(Literal::int(i128::from(*value))),
        ),
        other => (
            rust::Type::Ref {
                target: AllocBox::new(rust::Type::Primitive(rust::Primitive::Str)),
                mutable: false,
                lifetime: Some(Ident::new("static")),
            },
            rust::Expr::Literal(Literal::string(&rust_name(traits, other))),
        ),
    }
}

fn rust_name(traits: &TypeTraits, definition: &TraitDefinition) -> String {
    match definition {
        TraitDefinition::SelfType => zerodds_lang_render::rust::type_(
            &crate::stub::rust_param_type(&traits.subject, ParamDirection::In),
        ),
        TraitDefinition::Passing(direction) => zerodds_lang_render::rust::type_(
            &crate::stub::rust_param_type(&traits.subject, *direction),
        ),
        TraitDefinition::TraitsOf(name) => name.clone(),
        TraitDefinition::Derived(form) => derived_idl_name(traits, *form),
        TraitDefinition::Flag(value) => value.to_string(),
        TraitDefinition::Constant(value) => value.to_string(),
    }
}

/// A derived member's name, for the three columns that carry names.
///
/// The IDL-level answer and not a C++ one: a Java or C# reader of
/// `REF_TYPE` wants the type the mapping gives a reference, and
/// `omg::types::ref_type<T>` is a C++ spelling neither language has.
fn derived_idl_name(traits: &TypeTraits, form: DerivedForm) -> String {
    let subject = traits.display_name();
    match form {
        DerivedForm::Ref | DerivedForm::Factory => subject,
        DerivedForm::WeakRef => format!("weak {subject}"),
        DerivedForm::Obv => obv_name(&traits.scoped_name).replace("::", "."),
        DerivedForm::Underlying(bits) => match bits {
            0..=8 => "uint8".to_string(),
            9..=16 => "uint16".to_string(),
            17..=32 => "uint32".to_string(),
            _ => "uint64".to_string(),
        },
    }
}

// ---------------------------------------------------------------------
// Name conventions.
// ---------------------------------------------------------------------

/// `is_bounded` as `IS_BOUNDED`.
fn screaming_snake(name: &str) -> String {
    name.to_uppercase()
}

/// `is_bounded` as `IsBounded`.
fn pascal(name: &str) -> String {
    let mut out = String::new();
    let mut rising = true;
    for character in name.chars() {
        if character == '_' {
            rising = true;
            continue;
        }
        if rising {
            out.extend(character.to_uppercase());
            rising = false;
        } else {
            out.push(character);
        }
    }
    out
}

/// `SCorbaTraits` as `s_corba_traits`.
fn snake(name: &str) -> String {
    let mut out = String::new();
    for (index, character) in name.chars().enumerate() {
        if character.is_uppercase() {
            if index > 0 {
                out.push('_');
            }
            out.extend(character.to_lowercase());
        } else {
            out.push(character);
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn table_a1_gives_four_members_to_a_plain_structure() {
        let traits = TypeTraits {
            scoped_name: "probe::Point".to_string(),
            subject: StubType::other("probe::Point", TypeShape::FixedAggregate),
            members: common_members(),
        };
        assert_eq!(
            traits.member_names(),
            alloc::vec!["value_type", "in_type", "out_type", "inout_type"]
        );
        // Table A.2's `is_local` is an interface's and nobody else's.
        assert!(traits.member("is_local").is_none());
    }

    #[test]
    fn a_structured_type_takes_table_a14_and_not_a13() {
        let traits = TypeTraits {
            scoped_name: "probe::Point".to_string(),
            subject: StubType::other("probe::Point", TypeShape::FixedAggregate),
            members: common_members(),
        };
        let text = render_type_traits(&traits, TargetLanguage::Cpp).expect("cpp");
        assert!(
            text.contains("using in_type = const ::probe::Point&;"),
            "{text}"
        );
        assert!(text.contains("using out_type = ::probe::Point&;"), "{text}");
        assert!(
            text.contains("using inout_type = ::probe::Point&;"),
            "{text}"
        );
    }

    #[test]
    fn the_specialisation_argument_reaches_the_rendered_head() {
        let traits = TypeTraits {
            scoped_name: "probe::Point".to_string(),
            subject: StubType::other("probe::Point", TypeShape::FixedAggregate),
            members: common_members(),
        };
        let text = render_type_traits(&traits, TargetLanguage::Cpp).expect("cpp");
        assert!(
            text.contains("struct traits<::probe::Point>"),
            "the specialisation argument was dropped:\n{text}"
        );
    }

    #[test]
    fn an_object_reference_takes_table_a13_through_its_own_ref_type() {
        let traits = TypeTraits {
            scoped_name: "probe::Echo".to_string(),
            subject: StubType::other("probe::Echo", TypeShape::ObjectReference),
            members: {
                let mut members = common_members();
                members.push(TraitMember {
                    name: "ref_type",
                    definition: TraitDefinition::Derived(DerivedForm::Ref),
                });
                members
            },
        };
        let text = render_type_traits(&traits, TargetLanguage::Cpp).expect("cpp");
        assert!(text.contains("using in_type = ref_type;"), "{text}");
        assert!(text.contains("using out_type = ref_type&;"), "{text}");
        assert!(
            text.contains("using ref_type = omg::types::ref_type<::probe::Echo>;"),
            "{text}"
        );
    }

    #[test]
    fn a_flag_is_a_type_in_cpp_and_a_bool_in_the_other_three() {
        let traits = TypeTraits {
            scoped_name: "probe::Local".to_string(),
            subject: StubType::other("probe::Local", TypeShape::ObjectReference),
            members: alloc::vec![TraitMember {
                name: "is_local",
                definition: TraitDefinition::Flag(true),
            }],
        };
        assert!(
            render_type_traits(&traits, TargetLanguage::Cpp)
                .expect("cpp")
                .contains("using is_local = std::true_type;")
        );
        assert!(
            render_type_traits(&traits, TargetLanguage::Java)
                .expect("java")
                .contains("boolean IS_LOCAL = true;")
        );
        assert!(
            render_type_traits(&traits, TargetLanguage::CSharp)
                .expect("csharp")
                .contains("bool IsLocal = true;")
        );
        assert!(
            render_type_traits(&traits, TargetLanguage::Rust)
                .expect("rust")
                .contains("IS_LOCAL: bool = true;")
        );
    }

    #[test]
    fn a_bound_is_an_integral_constant_in_cpp() {
        let traits = TypeTraits {
            scoped_name: "probe::Name".to_string(),
            subject: StubType::Special(SpecialType::String),
            members: alloc::vec![TraitMember {
                name: "bound",
                definition: TraitDefinition::Constant(32),
            }],
        };
        let text = render_type_traits(&traits, TargetLanguage::Cpp).expect("cpp");
        assert!(
            text.contains("using bound = std::integral_constant<uint32_t, 32>;"),
            "{text}"
        );
    }

    #[test]
    fn every_column_renders_every_definition_kind() {
        let traits = TypeTraits {
            scoped_name: "probe::V".to_string(),
            subject: StubType::other("probe::V", TypeShape::Valuetype),
            members: alloc::vec![
                TraitMember {
                    name: "value_type",
                    definition: TraitDefinition::SelfType,
                },
                TraitMember {
                    name: "in_type",
                    definition: TraitDefinition::Passing(ParamDirection::In),
                },
                TraitMember {
                    name: "is_abstract",
                    definition: TraitDefinition::Flag(false),
                },
                TraitMember {
                    name: "bound",
                    definition: TraitDefinition::Constant(7),
                },
                TraitMember {
                    name: "element_traits",
                    definition: TraitDefinition::TraitsOf("probe::Element".to_string()),
                },
                TraitMember {
                    name: "obv_type",
                    definition: TraitDefinition::Derived(DerivedForm::Obv),
                },
                TraitMember {
                    name: "underlying_type",
                    definition: TraitDefinition::Derived(DerivedForm::Underlying(9)),
                },
            ],
        };
        for language in TargetLanguage::ALL {
            let text = render_type_traits(&traits, language).expect("renderable");
            assert!(!text.is_empty(), "{language:?}");
            for member in &traits.members {
                let spelled = match language {
                    TargetLanguage::Cpp => member.name.to_string(),
                    TargetLanguage::Java | TargetLanguage::Rust => screaming_snake(member.name),
                    TargetLanguage::CSharp => pascal(member.name),
                };
                assert!(
                    text.contains(&spelled),
                    "{language:?} dropped {spelled}:\n{text}"
                );
            }
        }
    }

    #[test]
    fn the_obv_prefix_goes_on_the_innermost_segment() {
        assert_eq!(obv_name("A::B::V"), "A::B::OBV_V");
        assert_eq!(obv_name("V"), "OBV_V");
    }

    #[test]
    fn a_bitmasks_underlying_width_follows_its_bit_bound() {
        assert_eq!(underlying_primitive(1), cpp::Primitive::UInt8);
        assert_eq!(underlying_primitive(8), cpp::Primitive::UInt8);
        assert_eq!(underlying_primitive(9), cpp::Primitive::UInt16);
        assert_eq!(underlying_primitive(32), cpp::Primitive::UInt32);
        assert_eq!(underlying_primitive(33), cpp::Primitive::UInt64);
        assert_eq!(underlying_primitive(64), cpp::Primitive::UInt64);
    }

    #[test]
    fn the_companion_name_drops_the_scope() {
        let traits = TypeTraits {
            scoped_name: "A::B::S".to_string(),
            subject: StubType::other("A::B::S", TypeShape::FixedAggregate),
            members: Vec::new(),
        };
        assert_eq!(traits.companion_name().as_deref(), Some("SCorbaTraits"));
        assert_eq!(snake("SCorbaTraits"), "s_corba_traits");
    }
}
