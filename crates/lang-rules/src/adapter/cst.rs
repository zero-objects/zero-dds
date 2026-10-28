// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The language CST as the other end of a rule. Spec §3.2.
//!
//! A rule's R-side creates seesaw nodes; this materialises them as CST
//! nodes. Creation is total: a seesaw node whose type has no CST
//! counterpart is an error at apply time, not a node silently dropped.
//!
//! Values are resolved from the model, never from the overlay. A rule
//! that carried a name through the graph would put that name into a hash
//! and make renaming a member change the identity of everything
//! downstream. Spec §2.3.

use crate::adapter::constant;
use crate::apply::Applied;
use seesaw_tgg::ident::GhostId;
use zerodds_ast::{Ast, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::{Id, NodeId};
use zerodds_lang_cst::common::{Doc, Ident, Path, Visibility};
use zerodds_lang_cst::rust;

/// Why a created node could not be materialised.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaterialiseError {
    /// A created node's type has no CST counterpart.
    UnknownType {
        /// The type name the rule created.
        type_name: String,
    },
    /// A created node has no model node behind it.
    ///
    /// Every CST node needs its values, and the values come from the
    /// model; a node with no origin has nowhere to get them.
    NoOrigin {
        /// The type name of the node.
        type_name: String,
    },
    /// The module an item nests inside cannot be read from the model.
    ///
    /// Reported rather than absorbed. A module id the arena does not
    /// answer for, or a parent chain deeper than `MAX_MODULE_DEPTH`,
    /// used to return the items **unwrapped** — `NameComponent` at the
    /// crate root instead of inside `pub mod CosNaming`. The scope is
    /// part of the declaration: `module a { struct P; };` and
    /// `module b { struct P; };` are two types, and flattening them is
    /// the same class of substitution as writing the wrong octet.
    NoModule {
        /// The module handle, as far as it can be named without the
        /// node the model would not hand over.
        at: String,
    },
    /// A child the model listed and then would not hand over.
    ///
    /// A walk asks the model for a type's members, cases, enumerators or
    /// bits and then reads each one by its handle. A handle that does
    /// not resolve means the model contradicts itself, and the only
    /// honest answer is to say so: skipping the child emits an
    /// aggregate with a field missing, a union with an arm missing or an
    /// enumeration with a value missing, and reports success. Nothing
    /// downstream can tell such a type from one the source declared that
    /// way — it is the same class of substitution as writing the wrong
    /// octet, and it reaches the wire.
    NoNode {
        /// What was asked for: `"member"`, `"case"`, `"enumerator"`,
        /// `"bitfield"`, `"bit"`, `"operation"` or `"parameter"`.
        kind: &'static str,
        /// Where it sits, as far as the model names it.
        at: String,
    },
    /// A value the output has to state does not evaluate.
    ///
    /// An array length, a constant's value, an enumerator's value or a
    /// bit's position. Substituting a zero would put a plausible number
    /// in the generated source — a `[T; 0]` where an array belongs, or
    /// two enumerators sharing a discriminant — and nothing downstream
    /// could tell it from a number the source asked for.
    Unevaluated {
        /// What could not be worked out, in the source's terms.
        construct: String,
        /// Where it sits, as far as the model names it.
        at: String,
    },
    /// A construct the model holds and the target language cannot.
    ///
    /// Named rather than skipped. A generator that left the member out
    /// would emit a type missing a field and report success; one that
    /// wrote the construct's IDL spelling into the output would emit a
    /// file no compiler reads and report success. Both are the same
    /// failure and this is the third answer.
    NoLanguageForm {
        /// The construct, in the source's terms.
        construct: String,
        /// The language that has no form for it.
        language: &'static str,
        /// Where it sits, as far as the model names it.
        at: String,
    },
    /// Two declarations in one scope spell the same name in the target.
    ///
    /// Every column escapes a name that collides with a keyword of its
    /// language by appending an underscore, so an IDL `int` becomes a C
    /// `int_`. A scope that already declares `int_` then has two
    /// members with one name, and the escape has silently made one of
    /// them unreachable.
    ///
    /// What each column does with that today is not the same, and none
    /// of it names the cause: the C compiler says `duplicate member
    /// 'int_'`, Python says `keyword argument repeated`, and a column
    /// whose target permits the duplicate says nothing at all — the
    /// second declaration wins and one member is dropped from a file
    /// that compiles. The escape is the only layer that holds both
    /// spellings at once, which is why the check is here and not in
    /// seven backends.
    SpellingCollision {
        /// The two IDL names, in declaration order.
        first: String,
        /// The second, which escaped onto the first.
        second: String,
        /// The spelling they share.
        spelled: String,
        /// The language whose keyword forced the escape.
        language: &'static str,
        /// The scope holding both.
        at: String,
    },
}

impl core::fmt::Display for MaterialiseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NoNode { kind, at } => {
                write!(f, "{at}: the model lists a {kind} it will not hand over")
            }
            Self::Unevaluated { construct, at } => {
                write!(f, "{at}: the {construct} does not evaluate")
            }
            Self::NoLanguageForm {
                construct,
                language,
                at,
            } => write!(f, "{at}: {language} has no form for `{construct}`"),
            Self::UnknownType { type_name } => {
                write!(
                    f,
                    "the rules created a `{type_name}`, which is not a CST node"
                )
            }
            Self::NoOrigin { type_name } => {
                write!(f, "a created `{type_name}` has no model node behind it")
            }
            Self::SpellingCollision {
                first,
                second,
                spelled,
                language,
                at,
            } => write!(
                f,
                "{at}: `{first}` and `{second}` both spell `{spelled}` in \
                 {language}, so one of them would be unreachable"
            ),
            Self::NoModule { at } => {
                write!(
                    f,
                    "the module {at} is not in the model, so the items it \
                     declares have no scope to nest in"
                )
            }
        }
    }
}

impl core::error::Error for MaterialiseError {}

/// The names a scope declares, checked for a collision the escape made.
///
/// `spelled` is what the target writes, `declared` is what the IDL
/// said. A column calls this once per scope, before it builds the
/// members, and gets the pair back rather than a file with a name
/// missing.
///
/// Order matters and is the model's: the error names the earlier
/// declaration first, so two runs over one model report the same pair.
///
/// # Errors
/// [`MaterialiseError::SpellingCollision`] when two declared names
/// reach one spelling.
pub fn no_spelling_collision<'a>(
    names: impl IntoIterator<Item = (&'a str, &'a str)>,
    language: &'static str,
    at: &str,
) -> Result<(), MaterialiseError> {
    let mut seen: Vec<(&str, &str)> = Vec::new();
    for (spelled, declared) in names {
        if let Some((_, first)) = seen.iter().find(|(other, _)| *other == spelled) {
            return Err(MaterialiseError::SpellingCollision {
                first: (*first).to_string(),
                second: declared.to_string(),
                spelled: spelled.to_string(),
                language,
                at: at.to_string(),
            });
        }
        seen.push((spelled, declared));
    }
    Ok(())
}

/// Build the Rust items a rule application produced.
///
/// The order is the model's own declaration order, so two runs over one
/// model produce the same file.
///
/// # Errors
/// [`MaterialiseError`] when a created node has no counterpart or no
/// origin — never a silently dropped node.
pub fn rust_items(ast: &Ast, applied: &Applied) -> Result<Vec<rust::Item>, MaterialiseError> {
    Ok(crate::verbatim::weave_file(
        ast,
        crate::verbatim::aliases::RUST,
        nest_by_module(ast, rust_item_pairs(ast, applied)?)?,
    ))
}

/// Weave every type's `@verbatim` text into the items the type walk
/// declared. `starts` holds, per type in walk order, where its items
/// begin; they end where the next type's begin. Woven last to first, so
/// an index still to be used never moves.
///
/// `BEGIN_DECLARATION` and `END_DECLARATION` open and close the body of
/// the type's `struct`; an enum and a type alias take them adjacent.
fn weave_rust_verbatim(
    ast: &Ast,
    starts: &[(Id<TypeNode>, Id<zerodds_ast::ModuleNode>, usize)],
    items: &mut Vec<(Id<zerodds_ast::ModuleNode>, rust::Item)>,
) {
    let mut end = items.len();
    for (id, module, start) in starts.iter().rev() {
        let slots = crate::verbatim::declaration_slots(ast, *id, crate::verbatim::aliases::RUST);
        let range_end = end;
        end = *start;
        if slots.is_empty() {
            continue;
        }
        let name = ast
            .type_(*id)
            .map(|node| rust::escape_keyword(&node.name))
            .unwrap_or_default();
        let tail = items.split_off(range_end.min(items.len()));
        let module = *module;
        crate::verbatim::weave_with(
            slots,
            *start,
            items,
            |text| (module, rust::Item::Verbatim(text)),
            |declared, begin, end| {
                for (_, item) in declared {
                    if crate::verbatim::Carrier::take_into_body(item, &name, begin, end) {
                        return;
                    }
                }
            },
        );
        items.extend(tail);
    }
}

/// `@verbatim` in the Rust column: `BEGIN_DECLARATION` and
/// `END_DECLARATION` open and close a struct's braced body; an enum and a
/// type alias take them adjacent.
impl crate::verbatim::Carrier for rust::Item {
    fn verbatim(text: String) -> Self {
        Self::Verbatim(text)
    }

    fn take_into_body(
        &mut self,
        name: &str,
        begin: &mut Vec<String>,
        end: &mut Vec<String>,
    ) -> bool {
        let Self::Struct(node) = self else {
            return false;
        };
        if node.name.as_str() != name || matches!(node.body, rust::StructBody::Tuple(_)) {
            return false;
        }
        node.body_text.begin.append(begin);
        node.body_text.end.append(end);
        true
    }
}

/// As [`rust_items`], each item paired with the module it belongs in and
/// nothing nested yet.
///
/// A caller that has items of its own for the same model — a build
/// script emitting codecs beside the types — must fold **one** list.
/// Folding two and concatenating declares every shared ancestor twice,
/// and Rust allows a name once per scope.
///
/// # Errors
/// As [`rust_items`].
pub fn rust_item_pairs(
    ast: &Ast,
    applied: &Applied,
) -> Result<Vec<(Id<zerodds_ast::ModuleNode>, rust::Item)>, MaterialiseError> {
    // Each item is kept with the module that declared it, and the
    // nesting is folded in at the end.
    //
    // **This used to be a flat `Vec<rust::Item>`, and the nesting was
    // dropped.** The catalogue has had a `module-to-mod` rule from the
    // start; the rule fires, the correspondence is created, and nothing
    // here ever turned it into a `rust::Item::Module` — so
    // `module a { struct P; }; module b { struct P; };` materialised as
    // two `pub struct P` in one scope, and every scoped name a consumer
    // wrote resolved to the wrong path or to nothing. Found by
    // `crates/corba-interop`, whose IDL declares `module CosNaming` and
    // whose consumers import `CosNaming::NameComponent`.
    let mut items: Vec<(Id<zerodds_ast::ModuleNode>, rust::Item)> = Vec::new();

    // Where each type's items begin, for the `@verbatim` pass after the
    // walk: the arms below leave by `continue` at many points, so the
    // text is woven once every type has declared what it declares.
    let mut starts: Vec<(Id<TypeNode>, Id<zerodds_ast::ModuleNode>, usize)> = Vec::new();

    // Walk the model rather than the graph: the model has the order the
    // source declared, and the graph has no order to speak of.
    for (id, node) in ast.types() {
        // Every type this item names is written relative to the module the
        // item itself lands in.
        let from = node.module;
        starts.push((id, from, items.len()));
        match &node.kind {
            TypeKind::Enumeration { enumerators } => {
                if ghost_created_for(applied, node.node_id, "RustEnum").is_none() {
                    continue;
                }
                let mut variants = Vec::new();
                for enumerator in enumerators {
                    let Ok(enum_node) = ast.enumerator(*enumerator) else {
                        continue;
                    };
                    if ghost_created_for(applied, enum_node.node_id, "RustVariant").is_none() {
                        continue;
                    }
                    // The value is the effective one, written out: a
                    // Rust enum without discriminants would renumber
                    // silently when a variant is inserted, and the wire
                    // format is the IDL's numbering, not Rust's.
                    let value = ast.effective_enumerator_value(*enumerator).ok_or_else(|| {
                        MaterialiseError::Unevaluated {
                            construct: "enumerator value".to_string(),
                            at: enum_node.name.clone(),
                        }
                    })?;
                    variants.push(rust::Variant {
                        doc: Doc::none(),
                        attrs: Vec::new(),
                        name: Ident::new(&rust::escape_keyword(&enum_node.name)),
                        body: rust::VariantBody::Unit,
                        discriminant: Some(rust::Expr::Literal(
                            zerodds_lang_cst::common::Literal::int(value),
                        )),
                    });
                }
                items.push((
                    node.module,
                    rust::Item::Enum(rust::Enum {
                        doc: Doc::none(),
                        attrs: Vec::new(),
                        // `Debug`, `Clone` and `PartialEq` and nothing
                        // else. The rule this file states — a derive is a
                        // decision about the target language and not about
                        // the wire — is about `Default`, whose Rust answer
                        // and Table 9's part company at an enumeration
                        // whose first literal is not zero. These three make
                        // no claim about the wire at all, and without them
                        // a generated type cannot be printed in an
                        // assertion, compared with another, or held twice.
                        // `crates/corba-interop`'s harness needs all three,
                        // and `Eq` is not among them because a `double`
                        // member rules it out.
                        derives: data_derives(),
                        visibility: Visibility::Public,
                        name: Ident::new(&rust::escape_keyword(&node.name)),
                        generics: Vec::new(),
                        variants,
                    }),
                ));
                continue;
            }
            TypeKind::Alias { aliased } => {
                if ghost_created_for(applied, node.node_id, "RustTypeAlias").is_none() {
                    continue;
                }
                // An anonymous alias is one the frontend made for an
                // inline type; it has no name to declare.
                if node.name.is_empty() {
                    continue;
                }
                items.push((
                    node.module,
                    rust::Item::TypeAlias(rust::TypeAlias {
                        doc: Doc::none(),
                        attrs: Vec::new(),
                        visibility: Visibility::Public,
                        name: Ident::new(&rust::escape_keyword(&node.name)),
                        generics: Vec::new(),
                        type_: rust_type_of(ast, from, aliased.resolved())?,
                    }),
                ));
                continue;
            }
            TypeKind::Union { cases, .. } => {
                if ghost_created_for(applied, node.node_id, "RustUnionEnum").is_none() {
                    continue;
                }
                // A union becomes an enum with one payload variant per
                // case: Rust's untagged union is unsafe, and the tag is
                // what the discriminator already is. Where the case does
                // not fix the discriminator, the variant holds it ahead
                // of the member, and a discriminator that may select no
                // member has a variant of its own (`crate::rust_union`).
                let shape = crate::rust_union::shape(ast, id).map_err(|reason| {
                    MaterialiseError::NoLanguageForm {
                        construct: reason.to_string(),
                        language: "rust",
                        at: node.name.clone(),
                    }
                })?;
                let discriminator_type = rust_type_of(ast, from, Some(shape.discriminator))?;
                let mut variants = Vec::new();
                for case in cases {
                    let Ok(case_node) = ast.case(*case) else {
                        continue;
                    };
                    if ghost_created_for(applied, case_node.node_id, "RustCaseVariant").is_none() {
                        continue;
                    }
                    let Ok(member) = ast.member(case_node.member) else {
                        continue;
                    };
                    let carries = shape
                        .cases
                        .iter()
                        .find(|c| c.member.node_id == member.node_id)
                        .is_some_and(crate::rust_union::RustCase::carries_discriminator);
                    let mut fields = Vec::new();
                    if carries {
                        fields.push(discriminator_type.clone());
                    }
                    fields.push(field_type(ast, from, member)?);
                    variants.push(rust::Variant {
                        doc: Doc::none(),
                        attrs: Vec::new(),
                        name: Ident::new(&variant_name(&member.name)),
                        body: rust::VariantBody::Tuple(fields),
                        discriminant: None,
                    });
                }
                if shape.no_member {
                    variants.push(rust::Variant {
                        doc: Doc::none(),
                        attrs: Vec::new(),
                        name: Ident::new(crate::rust_union::NO_MEMBER),
                        body: rust::VariantBody::Tuple(vec![discriminator_type]),
                        discriminant: None,
                    });
                }
                items.push((
                    node.module,
                    rust::Item::Enum(rust::Enum {
                        doc: Doc::none(),
                        attrs: Vec::new(),
                        // `Debug`, `Clone` and `PartialEq` and nothing
                        // else. The rule this file states — a derive is a
                        // decision about the target language and not about
                        // the wire — is about `Default`, whose Rust answer
                        // and Table 9's part company at an enumeration
                        // whose first literal is not zero. These three make
                        // no claim about the wire at all, and without them
                        // a generated type cannot be printed in an
                        // assertion, compared with another, or held twice.
                        // `crates/corba-interop`'s harness needs all three,
                        // and `Eq` is not among them because a `double`
                        // member rules it out.
                        derives: data_derives(),
                        visibility: Visibility::Public,
                        name: Ident::new(&rust::escape_keyword(&node.name)),
                        generics: Vec::new(),
                        variants,
                    }),
                ));
                continue;
            }
            TypeKind::Bitmask { bits } => {
                if ghost_created_for(applied, node.node_id, "RustBitmask").is_none() {
                    continue;
                }
                // A bitmask becomes the integer its bit bound calls for,
                // with one constant per bit: a Rust enum could not hold
                // two bits at once, which is the point of a bitmask.
                let width = ast.effective_bit_bound(id);
                items.push((
                    node.module,
                    rust::Item::TypeAlias(rust::TypeAlias {
                        doc: Doc::none(),
                        attrs: Vec::new(),
                        visibility: Visibility::Public,
                        name: Ident::new(&rust::escape_keyword(&node.name)),
                        generics: Vec::new(),
                        type_: rust::Type::Primitive(bitmask_primitive(width)),
                    }),
                ));
                for bit in bits {
                    let Ok(bit_node) = ast.bit_value(*bit) else {
                        continue;
                    };
                    if ghost_created_for(applied, bit_node.node_id, "RustBitConst").is_none() {
                        continue;
                    }
                    let position = ast.effective_bit_position(*bit).ok_or_else(|| {
                        MaterialiseError::Unevaluated {
                            construct: "bit position".to_string(),
                            at: bit_node.name.clone(),
                        }
                    })?;
                    items.push((
                        node.module,
                        rust::Item::Const(rust::Const {
                            doc: Doc::none(),
                            attrs: Vec::new(),
                            visibility: Visibility::Public,
                            name: Ident::new(&rust::escape_keyword(&bit_node.name)),
                            type_: rust::Type::Primitive(bitmask_primitive(width)),
                            value: rust::Expr::Binary {
                                op: rust::BinaryOp::Shl,
                                lhs: Box::new(rust::Expr::Literal(
                                    zerodds_lang_cst::common::Literal::int(1),
                                )),
                                rhs: Box::new(rust::Expr::Literal(
                                    zerodds_lang_cst::common::Literal::int(i128::from(position)),
                                )),
                            },
                        }),
                    ));
                }
                continue;
            }
            TypeKind::Bitset { .. } => {
                if ghost_created_for(applied, node.node_id, "RustBitset").is_none() {
                    continue;
                }
                // Each bitfield becomes a field of the holder type its
                // width calls for, the base's included; the packing is
                // the layout's business and not the source's shape.
                let mut struct_fields = Vec::new();
                for field in &ast.all_bitfields_of(id) {
                    let Ok(field_node) = ast.bitfield(*field) else {
                        continue;
                    };
                    // An anonymous bitfield is padding, and padding has
                    // no name to declare.
                    let Some(name) = field_node.name.as_ref() else {
                        continue;
                    };
                    struct_fields.push(rust::Field {
                        doc: Doc::none(),
                        attrs: Vec::new(),
                        visibility: Visibility::Public,
                        name: Ident::new(&rust::escape_keyword(name)),
                        type_: rust::Type::Primitive(bitmask_primitive(field_node.bits)),
                    });
                }
                items.push((
                    node.module,
                    rust::Item::Struct(rust::Struct {
                        body_text: zerodds_lang_cst::common::BodyText::none(),
                        doc: Doc::none(),
                        attrs: Vec::new(),
                        // `Debug`, `Clone` and `PartialEq` and nothing
                        // else. The rule this file states — a derive is a
                        // decision about the target language and not about
                        // the wire — is about `Default`, whose Rust answer
                        // and Table 9's part company at an enumeration
                        // whose first literal is not zero. These three make
                        // no claim about the wire at all, and without them
                        // a generated type cannot be printed in an
                        // assertion, compared with another, or held twice.
                        // `crates/corba-interop`'s harness needs all three,
                        // and `Eq` is not among them because a `double`
                        // member rules it out.
                        derives: data_derives(),
                        visibility: Visibility::Public,
                        name: Ident::new(&rust::escape_keyword(&node.name)),
                        generics: Vec::new(),
                        body: rust::StructBody::Named(struct_fields),
                    }),
                ));
                continue;
            }
            TypeKind::Valuetype { state, .. } => {
                if ghost_created_for(applied, node.node_id, "RustValueStruct").is_none() {
                    continue;
                }
                let mut struct_fields = Vec::new();
                for member in state {
                    let Ok(member_node) = ast.member(*member) else {
                        continue;
                    };
                    // A valuetype's state members are `Ast::members_of`
                    // just as a structure's are, so the planner frames an
                    // `@optional` one the same way and the field has to
                    // hold the same absence.
                    struct_fields.push(rust::Field {
                        doc: Doc::none(),
                        attrs: Vec::new(),
                        visibility: Visibility::Public,
                        name: Ident::new(&rust::escape_keyword(&member_node.name)),
                        type_: field_type(ast, from, member_node)?,
                    });
                }
                items.push((
                    node.module,
                    rust::Item::Struct(rust::Struct {
                        body_text: zerodds_lang_cst::common::BodyText::none(),
                        doc: Doc::none(),
                        attrs: Vec::new(),
                        // `Debug`, `Clone` and `PartialEq` and nothing
                        // else. The rule this file states — a derive is a
                        // decision about the target language and not about
                        // the wire — is about `Default`, whose Rust answer
                        // and Table 9's part company at an enumeration
                        // whose first literal is not zero. These three make
                        // no claim about the wire at all, and without them
                        // a generated type cannot be printed in an
                        // assertion, compared with another, or held twice.
                        // `crates/corba-interop`'s harness needs all three,
                        // and `Eq` is not among them because a `double`
                        // member rules it out.
                        derives: data_derives(),
                        visibility: Visibility::Public,
                        name: Ident::new(&rust::escape_keyword(&node.name)),
                        generics: Vec::new(),
                        body: rust::StructBody::Named(struct_fields),
                    }),
                ));
                continue;
            }
            TypeKind::Native => {
                if ghost_created_for(applied, node.node_id, "RustNativeAlias").is_none() {
                    continue;
                }
                // A native type is opaque: the alias names it and the
                // mapping to a concrete type comes from the facet, not
                // from a guess made here.
                items.push((
                    node.module,
                    rust::Item::TypeAlias(rust::TypeAlias {
                        doc: Doc::none(),
                        attrs: Vec::new(),
                        visibility: Visibility::Public,
                        name: Ident::new(&rust::escape_keyword(&node.name)),
                        generics: Vec::new(),
                        type_: native_type(ast, id),
                    }),
                ));
                continue;
            }
            TypeKind::Structure { .. } => {}
            _ => continue,
        }
        if ghost_created_for(applied, node.node_id, "RustStruct").is_none() {
            continue;
        }

        // XTypes 1.3 §7.5.1.4.1.3: a language with no mapping of its own for
        // inheritance binds the derived structure "as if an instance of the base type
        // were the first member of the subtype with the name 'parent'". The wire
        // still carries the base's members first (§7.4.1.1.5.1): the codec reaches
        // them through `parent`.
        let mut fields = Vec::new();
        let mut spelled = Vec::new();
        if let Some(base) = crate::adapter::c::struct_base(ast, id) {
            spelled.push((PARENT.to_string(), PARENT.to_string()));
            fields.push(rust::Field {
                doc: Doc::line("The base structure, the first member (XTypes 1.3 §7.5.1.4.1.3)."),
                attrs: Vec::new(),
                visibility: Visibility::Public,
                name: Ident::new(PARENT),
                type_: rust_type_of(ast, from, Some(base))?,
            });
        }
        for member in ast.members_of(id) {
            let Ok(member_node) = ast.member(*member) else {
                continue;
            };
            if ghost_created_for(applied, member_node.node_id, "RustField").is_none() {
                continue;
            }
            let field = rust::escape_keyword(&member_node.name);
            spelled.push((field.clone(), member_node.name.clone()));
            fields.push(rust::Field {
                doc: Doc::none(),
                attrs: Vec::new(),
                visibility: Visibility::Public,
                name: Ident::new(&field),
                type_: field_type(ast, from, member_node)?,
            });
        }
        no_spelling_collision(
            spelled
                .iter()
                .map(|(field, declared)| (field.as_str(), declared.as_str())),
            "Rust",
            &node.name,
        )?;

        items.push((
            node.module,
            rust::Item::Struct(rust::Struct {
                body_text: zerodds_lang_cst::common::BodyText::none(),
                doc: Doc::none(),
                attrs: Vec::new(),
                // `Debug`, `Clone` and `PartialEq` and nothing
                // else. The rule this file states — a derive is a
                // decision about the target language and not about
                // the wire — is about `Default`, whose Rust answer
                // and Table 9's part company at an enumeration
                // whose first literal is not zero. These three make
                // no claim about the wire at all, and without them
                // a generated type cannot be printed in an
                // assertion, compared with another, or held twice.
                // `crates/corba-interop`'s harness needs all three,
                // and `Eq` is not among them because a `double`
                // member rules it out.
                derives: data_derives(),
                visibility: Visibility::Public,
                name: Ident::new(&rust::escape_keyword(&node.name)),
                generics: Vec::new(),
                body: rust::StructBody::Named(fields),
            }),
        ));
    }

    weave_rust_verbatim(ast, &starts, &mut items);

    // Constants, exceptions and interfaces are not types, so they are
    // walked separately — the model keeps them in arenas of their own.
    for (id, node) in ast.constants() {
        // Every type this item names is written relative to the module the
        // item itself lands in.
        let from = node.module;
        if ghost_created_for(applied, node.node_id, "RustConst").is_none() {
            continue;
        }
        // `eval_const` and not `eval_integral`: `const string NAME`
        // evaluates perfectly well and is not a number, and asking the
        // integral question about it answered `Unevaluated` and dropped
        // the item.
        let value = ast
            .eval_const(node.expr)
            .map_err(|_| MaterialiseError::Unevaluated {
                construct: "constant's value".to_string(),
                at: node.name.clone(),
            })?;
        let declared = rust_type_of(ast, from, node.type_.resolved())?;
        let (type_, literal) = rust_constant(&value, declared, &node.name)?;
        items.push((
            node.module,
            rust::Item::Const(rust::Const {
                doc: Doc::none(),
                attrs: Vec::new(),
                visibility: Visibility::Public,
                name: Ident::new(&rust::escape_keyword(&node.name)),
                type_,
                value: rust::Expr::Literal(literal),
            }),
        ));
        let _ = id;
    }

    for (id, node) in ast.exceptions() {
        // Every type this item names is written relative to the module the
        // item itself lands in.
        let from = node.module;
        if ghost_created_for(applied, node.node_id, "RustErrorStruct").is_none() {
            continue;
        }
        let mut fields = Vec::new();
        for member in &node.members {
            let Ok(member_node) = ast.member(*member) else {
                continue;
            };
            fields.push(rust::Field {
                doc: Doc::line(&format!("The IDL exception member `{}`.", member_node.name)),
                attrs: Vec::new(),
                visibility: Visibility::Public,
                name: Ident::new(&rust::escape_keyword(&member_node.name)),
                type_: rust_type_of(ast, from, member_node.type_.resolved())?,
            });
        }
        items.push((
            node.module,
            rust::Item::Struct(rust::Struct {
                body_text: zerodds_lang_cst::common::BodyText::none(),
                doc: Doc::line(&format!(
                    "The IDL exception `{}`, as `Err` payload — DDS-RPC 1.0 \
                     §7.5.1.1.5 gives each raised exception its own case in \
                     the operation's `_Result` union.",
                    node.name
                )),
                attrs: Vec::new(),
                // `Debug`, `Clone` and `PartialEq` and nothing
                // else. The rule this file states — a derive is a
                // decision about the target language and not about
                // the wire — is about `Default`, whose Rust answer
                // and Table 9's part company at an enumeration
                // whose first literal is not zero. These three make
                // no claim about the wire at all, and without them
                // a generated type cannot be printed in an
                // assertion, compared with another, or held twice.
                // `crates/corba-interop`'s harness needs all three,
                // and `Eq` is not among them because a `double`
                // member rules it out.
                derives: data_derives(),
                visibility: Visibility::Public,
                name: Ident::new(&rust::escape_keyword(&node.name)),
                generics: Vec::new(),
                body: rust::StructBody::Named(fields),
            }),
        ));
        let _ = id;
    }

    for (id, node) in ast.interfaces() {
        // Every type this item names is written relative to the module the
        // item itself lands in.
        let from = node.module;
        if ghost_created_for(applied, node.node_id, "RustTrait").is_none() {
            continue;
        }
        // An interface becomes a trait of signatures. The transport is
        // the RPC arc's business; what belongs here is the shape the
        // IDL declared.
        //
        // The whole answer and not the declared one: `supertraits` below
        // is empty, so a derived interface's trait has no supertrait to
        // reach its bases' operations through and OMG IDL 4.2 §7.4.3's
        // inherited operations are written into it. See
        // [`Ast::all_operations_of`].
        let mut methods = Vec::new();
        for operation in &ast.all_operations_of(id) {
            let Ok(op) = ast.operation(*operation) else {
                continue;
            };
            let mut params = vec![rust::Param {
                name: None,
                type_: rust::Type::unit(),
                receiver: Some(rust::Receiver::Ref),
            }];
            for param in &op.params {
                let Ok(param_node) = ast.param(*param) else {
                    continue;
                };
                params.push(rust::Param {
                    name: Some(Ident::new(&rust::escape_keyword(&param_node.name))),
                    type_: rust_type_of(ast, from, param_node.type_.resolved())?,
                    receiver: None,
                });
            }
            methods.push(rust::Item::Function(rust::Function {
                doc: Doc::none(),
                attrs: Vec::new(),
                visibility: Visibility::Private,
                name: Ident::new(&rust::escape_keyword(&op.name)),
                generics: Vec::new(),
                params,
                returns: op
                    .return_type
                    .as_ref()
                    .map(|type_| rust_type_of(ast, from, type_.resolved()))
                    .transpose()?,
                body: None,
            }));
        }
        items.push((
            node.module,
            rust::Item::Trait(rust::Trait {
                doc: Doc::none(),
                attrs: Vec::new(),
                visibility: Visibility::Public,
                name: Ident::new(&rust::escape_keyword(&node.name)),
                generics: Vec::new(),
                supertraits: Vec::new(),
                items: methods,
            }),
        ));
    }

    Ok(items)
}

/// The declarations, each inside the `pub mod` its IDL module names.
///
/// Public because the codecs are a second call —
/// `crate::encode::rust_codec`, once per type — and they have to land
/// in the same module as the declaration they are an `impl` of. A
/// caller that emitted both and nested only the first would produce an
/// `impl` for a name that is not in scope.
///
/// The root module is the anonymous global scope and produces no
/// `mod`; every other one nests, outermost first, and a module with no
/// materialised declaration produces nothing — an empty `pub mod` is a
/// name in the output that nothing put there.
///
/// Declaration order is preserved within a module, and the modules
/// follow the order their first declaration appeared in, so the output
/// tracks the source rather than a hash.
///
/// # Errors
/// [`MaterialiseError::NoModule`] when the model does not answer for a
/// module an item names. Returning the item unwrapped instead put it in
/// the enclosing scope and said nothing.
pub fn nest_by_module(
    ast: &Ast,
    items: Vec<(Id<zerodds_ast::ModuleNode>, rust::Item)>,
) -> Result<Vec<rust::Item>, MaterialiseError> {
    // Each item is placed by its **whole** module path and not by its
    // innermost module. Grouping by the innermost one and wrapping each
    // group in its own chain gives two declarations of every shared
    // ancestor: `spatial::core::A` and `spatial::sensing::B` became
    // `mod spatial { mod core { A } }` beside `mod spatial { mod sensing
    // { B } }`, and Rust — unlike IDL, where §7.5 lets a module be
    // reopened — allows a name once per scope. The generated file did
    // not compile: `the name `spatial` is defined multiple times`.
    let mut entries: Vec<PathedItem> = Vec::with_capacity(items.len());
    for (module, item) in items {
        entries.push((module_path(ast, module)?, item));
    }
    fold_by_path(ast, entries, 0)
}

/// A declared type's path as written from inside module `from`.
///
/// The simple name alone only resolves inside the type's own module. A
/// member of `spatial::sensing::FrameRef` whose type is
/// `spatial::core::CoordConvention` was emitted as `CoordConvention`,
/// which does not resolve there: the folded module carries
/// `use super::*`, and that reaches the enclosing scope and not a
/// sibling.
///
/// The path is **relative** and not rooted at the crate. An absolute
/// `crate::a::B::C` was tried first and is wrong: it resolves for
/// `crates/spatial-dds`, whose `src/lib.rs` includes the generated file
/// at the crate root, and breaks `crates/wire-conformance`, which
/// includes its own inside a module — `cannot find type `Cell` in the
/// crate root`. A relative path does not depend on where the file is
/// included.
///
/// Same module means no prefix, which is why every generator that emits
/// one module's worth of types is unaffected by this.
///
/// # Errors
/// [`MaterialiseError::NoModule`] when the model does not answer for a
/// module in either chain.
pub(crate) fn relative_path(
    ast: &Ast,
    from: Id<zerodds_ast::ModuleNode>,
    node: &zerodds_ast::TypeNode,
) -> Result<Path, MaterialiseError> {
    let here = module_path(ast, from)?;
    let there = module_path(ast, node.module)?;
    let shared = here
        .iter()
        .zip(there.iter())
        .take_while(|(a, b)| a == b)
        .count();

    let mut segments: Vec<String> = Vec::new();
    // One `super` per module left behind…
    for _ in shared..here.len() {
        segments.push(String::from("super"));
    }
    // …then down into the ones the target adds.
    for module in there.iter().skip(shared) {
        let enclosing = ast
            .module(*module)
            .map_err(|_| MaterialiseError::NoModule {
                at: module_handle(*module),
            })?;
        segments.push(rust::escape_keyword(&enclosing.name));
    }
    segments.push(rust::escape_keyword(&node.name));
    let borrowed: Vec<&str> = segments.iter().map(String::as_str).collect();
    Ok(Path::new(&borrowed))
}

/// A declared type's path as written from the **top level of the
/// generated file** — `common::util::Location`.
///
/// The counterpart of `relative_path` for a caller that is not inside
/// one of the folded modules. `nest_by_module` puts every type under one
/// `pub mod` per IDL module, so an item appended after that tree — a
/// trait impl, say — names a type by the chain of modules it sits in.
///
/// Relative and not rooted at the crate, for the reason
/// `relative_path` gives: the generated file is included at a crate
/// root in one consumer and inside a module in another, and only a
/// relative path holds in both.
///
/// # Errors
/// [`MaterialiseError::NoModule`] when the model does not answer for a
/// module in the chain, or [`MaterialiseError::NoOrigin`] when it does
/// not answer for the type.
pub fn path_from_file_root(ast: &Ast, type_: Id<TypeNode>) -> Result<String, MaterialiseError> {
    let node = ast.type_(type_).map_err(|_| MaterialiseError::NoOrigin {
        type_name: format!("{type_:?}"),
    })?;
    let mut segments: Vec<String> = Vec::new();
    for module in module_path(ast, node.module)? {
        let enclosing = ast.module(module).map_err(|_| MaterialiseError::NoModule {
            at: module_handle(module),
        })?;
        segments.push(rust::escape_keyword(&enclosing.name));
    }
    segments.push(rust::escape_keyword(&node.name));
    Ok(segments.join("::"))
}

/// One item with the module path it belongs at, outermost first.
type PathedItem = (Vec<Id<zerodds_ast::ModuleNode>>, rust::Item);

/// The chain of modules enclosing `module`, outermost first, with the
/// unnamed root left out.
///
/// # Errors
/// [`MaterialiseError::NoModule`] when the model does not answer for a
/// module in the chain, or when the chain runs past
/// [`MAX_MODULE_DEPTH`].
pub(crate) fn module_path(
    ast: &Ast,
    module: Id<zerodds_ast::ModuleNode>,
) -> Result<Vec<Id<zerodds_ast::ModuleNode>>, MaterialiseError> {
    let mut path = Vec::new();
    let mut current = module;
    for _ in 0..=MAX_MODULE_DEPTH {
        let node = ast
            .module(current)
            .map_err(|_| MaterialiseError::NoModule {
                at: module_handle(current),
            })?;
        // The root has no name and is not a `mod`.
        let Some(parent) = node.parent else {
            path.reverse();
            return Ok(path);
        };
        path.push(current);
        current = parent;
    }
    Err(MaterialiseError::NoModule {
        at: format!(
            "{} — the parent chain runs past {MAX_MODULE_DEPTH}",
            module_handle(module)
        ),
    })
}

/// `entries`, folded into one `pub mod` per distinct module at `depth`.
///
/// Order is first appearance, at every level: a module is declared where
/// its first item put it, and every later item of the same module joins
/// that declaration rather than opening a second one.
///
/// zerodds-lint: recursion-depth 64 (module nesting; bounded by `MAX_MODULE_DEPTH`)
fn fold_by_path(
    ast: &Ast,
    entries: Vec<PathedItem>,
    depth: usize,
) -> Result<Vec<rust::Item>, MaterialiseError> {
    if depth > MAX_MODULE_DEPTH {
        return Err(MaterialiseError::NoModule {
            at: format!("a module chain runs past {MAX_MODULE_DEPTH}"),
        });
    }
    let mut here: Vec<rust::Item> = Vec::new();
    let mut order: Vec<Id<zerodds_ast::ModuleNode>> = Vec::new();
    let mut deeper: Vec<(Id<zerodds_ast::ModuleNode>, Vec<PathedItem>)> = Vec::new();

    for (path, item) in entries {
        match path.get(depth).copied() {
            None => here.push(item),
            Some(module) => match order.iter().position(|id| *id == module) {
                Some(at) => deeper[at].1.push((path, item)),
                None => {
                    order.push(module);
                    deeper.push((module, vec![(path, item)]));
                }
            },
        }
    }

    // Items that stop here keep their place ahead of the modules that
    // follow them, which is the order the source declared.
    let mut out = here;
    for (module, nested) in deeper {
        let node = ast.module(module).map_err(|_| MaterialiseError::NoModule {
            at: module_handle(module),
        })?;
        out.push(rust::Item::Module(rust::Module {
            // `missing_docs` fires on every `pub mod` a consumer's own
            // `-D warnings` turns on, same as it fires on the type this
            // module holds; one line here answers it for every column
            // that folds through this function.
            doc: Doc::line(&format!("The IDL module `{}`.", node.name)),
            attrs: Vec::new(),
            visibility: Visibility::Public,
            name: Ident::new(&rust::escape_keyword(&node.name)),
            // A module is a scope: everything the file imported, and every
            // type declared beside this module, has to be reachable from
            // inside it. `use super::*` is one line per module and reaches
            // both, which a list of named imports would not — a codec here
            // names types from any module the IDL let it name.
            uses: vec![rust::Use {
                path: Path::single("super"),
                names: vec![Ident::new("*")],
                visibility: Visibility::Private,
            }],
            items: fold_by_path(ast, nested, depth + 1)?,
        }));
    }
    Ok(out)
}

/// How deep a module chain is followed before the model is taken to be
/// malformed. IDL nests nowhere near this.
const MAX_MODULE_DEPTH: usize = 64;

/// How deep a type's Rust spelling may nest before the walk refuses it.
///
/// The same 64 every reference walk in the tree carries;
/// `zerodds-wire-codec`'s `default.rs` and `zerodds-types`'
/// `dynamic/type_.rs` both stop there.
const MAX_TYPE_DEPTH: usize = 64;

/// The module handle, named without the node the model would not answer
/// for. Slot and generation are what the arena was asked for.
pub(crate) fn module_handle(module: Id<zerodds_ast::ModuleNode>) -> String {
    handle_of(module)
}

/// A handle, named as far as it can be without the node behind it.
///
/// Used where the model would not hand a node over: there is no name to
/// report, because the name lives in the node. The slot and generation
/// are what the arena knows, and they are enough to find the entry that
/// went missing.
pub(crate) fn handle_of<T>(id: Id<T>) -> String {
    format!("#{}@{}", id.slot(), id.generation())
}

/// The field type one member declaration takes.
///
/// `Box<T>` for external storage, wrapped in `Option` when also optional.
/// Otherwise `Option<T>` for an optional member and `T` for an ordinary one.
/// DDS-XTypes 1.3 §7.2.2.4.4.4.7 makes the member's *value* optional and
/// not its type: "the member may or may not be present in a given
/// object of that type". A field of the member's own type has no way to
/// say "not present", so the column that generated one could only refuse
/// a stream — in either encoding version — that says the member is
/// absent, which is a sample the peers on the other three columns hand
/// back without comment.
///
/// This is a type-mapping decision and not a wire one. The bytes are
/// rule (19)'s parameter header of declared length zero and rule (20)'s
/// BOOLEAN `<is_present>` of `0`, both of which the encoder already
/// wrote; what changes is that the sample can now hold the absence they
/// describe.
///
/// # Errors
/// As [`rust_items`].
fn field_type(
    ast: &Ast,
    from: Id<zerodds_ast::ModuleNode>,
    member: &zerodds_ast::MemberNode,
) -> Result<rust::Type, MaterialiseError> {
    let mut inner = rust_type_of(ast, from, member.type_.resolved())?;
    if is_external(ast, member) {
        inner = rust::Type::generic(Path::single("Box"), vec![inner]);
    }
    if member.facets.is_optional() {
        return Ok(rust::Type::generic(Path::new(&["Option"]), vec![inner]));
    }
    Ok(inner)
}

/// How the Rust column holds an IDL `map`.
///
/// XTypes 1.3 §7.5.1.3.6: "In all languages for which no language-specific mapping is
/// specified, the language binding for map types shall be based on the equivalent IDL definition
/// given in 7.4.1.1.4", the sequence of `MapEntry` structures; that is the default.
/// [`RustMapForm::BTreeMap`] is the idiomatic container and a named option: it has the
/// operations of a map and loses the order the entries were put in, which is ascending by key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RustMapForm {
    /// `Vec<zerodds_wire::MapEntry<K, V>>`.
    #[default]
    Entries,
    /// `BTreeMap<K, V>`.
    BTreeMap,
}

thread_local! {
    static MAP_FORM: std::cell::Cell<RustMapForm> = const { std::cell::Cell::new(RustMapForm::Entries) };
}

/// Selects the form of a Rust map on the current thread until dropped.
///
/// A scope and not a parameter for the reason [`super::cpp::CppMappingScope`] gives.
#[derive(Debug)]
pub struct RustMapFormScope {
    previous: RustMapForm,
}

impl RustMapFormScope {
    /// Selects `form` on this thread.
    #[must_use]
    pub fn enter(form: RustMapForm) -> Self {
        let previous = MAP_FORM.with(|cell| cell.replace(form));
        Self { previous }
    }
}

impl Drop for RustMapFormScope {
    fn drop(&mut self) {
        MAP_FORM.with(|cell| cell.set(self.previous));
    }
}

/// The form of a Rust map in force on this thread.
#[must_use]
pub fn rust_map_form() -> RustMapForm {
    MAP_FORM.with(std::cell::Cell::get)
}

/// The name of the member that holds the base of a derived structure (XTypes 1.3 §7.5.1.4.1.3).
pub(crate) const PARENT: &str = "parent";

/// XTypes' external storage is independent of optional presence. Rust owns
/// that storage through a Box; Option, when present, wraps the Box.
///
/// A member that closes a type cycle is held the same way, annotated or
/// not ([`super::held_externally`]): `Option<Node>` inside `Node` is
/// `error[E0072]: recursive type has infinite size`, and the Box is the
/// indirection rustc's own help text asks for.
pub(crate) fn is_external(ast: &Ast, member: &zerodds_ast::MemberNode) -> bool {
    super::held_externally(ast, member)
}

pub(crate) fn external_value(
    ast: &Ast,
    member: &zerodds_ast::MemberNode,
    value: rust::Expr,
) -> rust::Expr {
    if is_external(ast, member) {
        rust::Expr::Call {
            callee: Box::new(rust::Expr::Path(Path::new(&["Box", "new"]))),
            args: vec![value],
        }
    } else {
        value
    }
}

/// The Rust variant name a union case member takes.
///
/// Rust variants are upper camel case and IDL members are not; the name
/// is the member's with its first letter raised, so the two stay
/// recognisably the same name.
pub(crate) fn variant_name(member: &str) -> String {
    let mut chars = member.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// The unsigned integer a bit width calls for.
pub(crate) const fn bitmask_primitive(bits: u16) -> rust::Primitive {
    match bits {
        0..=8 => rust::Primitive::U8,
        9..=16 => rust::Primitive::U16,
        17..=32 => rust::Primitive::U32,
        _ => rust::Primitive::U64,
    }
}

/// What a `native` declaration maps to.
///
/// The facet if a `#pragma` or an annotation gave one, and an opaque
/// pointer otherwise — never a guess at what the name might mean.
fn native_type(ast: &Ast, id: zerodds_ast_core::Id<zerodds_ast::TypeNode>) -> rust::Type {
    let mapping = ast
        .any_of_type(id)
        .ok()
        .and_then(|any| ast.facets(any))
        .and_then(|facets| facets.corba.as_ref())
        .and_then(|corba| corba.native_mapping.clone());
    match mapping {
        Some(name) => rust::Type::named(Path::single(&name)),
        None => rust::Type::Ref {
            target: Box::new(rust::Type::Primitive(rust::Primitive::U8)),
            mutable: false,
            lifetime: None,
        },
    }
}

/// Whether the rules created a node of `type_name` for `node`.
///
/// The correspondence the rule established anchors on the model node, so
/// the created node's identity is derived from it.
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
            applied.graph.connected(&anchor, created) || is_derived_from(applied, &anchor, created)
        })
}

/// Whether a created node hangs off `anchor` through a correspondence.
fn is_derived_from(applied: &Applied, anchor: &GhostId, created: &GhostId) -> bool {
    // A correspondence is an ordinary node with two links: one to the
    // anchor, one to what was created. Two hops rather than one.
    applied
        .graph
        .parts(anchor)
        .any(|part| applied.graph.connected(&part.other, created))
}

/// The Rust type a model type becomes.
///
/// Total over `TypeKind` with no catch-all: a kind added to the model
/// without a mapping here is a compile error rather than a field that
/// silently takes some other type.
///
/// `pub(crate)`: `emit::interface_rust` spells an operation parameter's
/// or an attribute's type the same way a member's is spelled — the two
/// are one clause apart (OMG IDL 4.2 §7.4.3) and a second reading would
/// disagree about, say, whether `long double` takes `zerodds_float128`.
pub(crate) fn rust_type_of(
    ast: &Ast,
    from: Id<zerodds_ast::ModuleNode>,
    target: Option<zerodds_ast_core::Id<zerodds_ast::TypeNode>>,
) -> Result<rust::Type, MaterialiseError> {
    type_of_at(ast, from, target, 0)
}

/// [`rust_type_of`], carrying how many connector steps the walk has
/// already taken.
///
/// The `Alias` arm follows every `typedef`, named ones included, so a
/// `typedef` cycle — a finding the model's invariant pass reports rather
/// than a shape it rejects — used to read forever here.
///
/// # Errors
/// [`MaterialiseError::Unevaluated`] when the spelling nests past
/// `MAX_TYPE_DEPTH`.
///
/// zerodds-lint: recursion-depth 64 (sequence, array, map, value-box and
/// typedef nesting; bounded by `MAX_TYPE_DEPTH`, which returns
/// `MaterialiseError::Unevaluated`)
fn type_of_at(
    ast: &Ast,
    from: Id<zerodds_ast::ModuleNode>,
    target: Option<zerodds_ast_core::Id<zerodds_ast::TypeNode>>,
    depth: usize,
) -> Result<rust::Type, MaterialiseError> {
    if depth > MAX_TYPE_DEPTH {
        return Err(MaterialiseError::Unevaluated {
            construct: format!("type, whose Rust spelling nests past {MAX_TYPE_DEPTH}"),
            at: target.map_or_else(|| "a member".to_string(), crate::adapter::cst::handle_of),
        });
    }
    let Some(target) = target else {
        // An unresolved member type is a frontend failure the model
        // already reports; the field takes the unit type so the shape of
        // the output still shows what is missing.
        return Ok(rust::Type::unit());
    };
    let Ok(node) = ast.type_(target) else {
        return Ok(rust::Type::unit());
    };
    Ok(match &node.kind {
        // `long double` is the one primitive with no Rust built-in.
        // `zerodds_float128::Float128` carries it exactly — the sixteen
        // bytes CDR gives it, with the conversions to and from `f64`
        // reporting the values that do not fit. It used to take `f64`,
        // which put a value nobody wrote on the wire.
        TypeKind::Primitive(PrimitiveKind::Float128) => {
            rust::Type::named(Path::new(&["zerodds_float128", "Float128"]))
        }
        TypeKind::Primitive(kind) => rust::Type::Primitive(rust_primitive(*kind)),
        TypeKind::String { .. } => rust::Type::Primitive(rust::Primitive::String),
        TypeKind::Sequence { element, .. } => rust::Type::generic(
            Path::new(&["Vec"]),
            vec![type_of_at(ast, from, element.resolved(), depth + 1)?],
        ),
        TypeKind::Array { element, dims } => {
            let mut out = type_of_at(ast, from, element.resolved(), depth + 1)?;
            // Innermost dimension first, so `long a[2][3]` becomes
            // `[[i32; 3]; 2]` — the outer dimension outermost.
            for dim in dims.iter().rev() {
                let length =
                    ast.eval_integral(*dim)
                        .map_err(|_| MaterialiseError::Unevaluated {
                            construct: "array length".to_string(),
                            at: named(&node.name, node.node_id),
                        })?;
                out = rust::Type::Array {
                    element: Box::new(out),
                    length: Box::new(rust::Expr::Literal(zerodds_lang_cst::common::Literal::int(
                        length,
                    ))),
                };
            }
            out
        }
        TypeKind::Map { key, value, .. } => {
            let key = type_of_at(ast, from, key.resolved(), depth + 1)?;
            let value = type_of_at(ast, from, value.resolved(), depth + 1)?;
            match rust_map_form() {
                RustMapForm::BTreeMap => {
                    rust::Type::generic(Path::new(&["BTreeMap"]), vec![key, value])
                }
                // XTypes 1.3 §7.5.1.3.6: the sequence of `MapEntry`.
                RustMapForm::Entries => rust::Type::generic(
                    Path::new(&["Vec"]),
                    vec![rust::Type::generic(
                        Path::new(&["zerodds_wire", "MapEntry"]),
                        vec![key, value],
                    )],
                ),
            }
        }
        TypeKind::Alias { aliased } => type_of_at(ast, from, aliased.resolved(), depth + 1)?,
        TypeKind::Structure { .. }
        | TypeKind::Union { .. }
        | TypeKind::Enumeration { .. }
        | TypeKind::Bitmask { .. }
        | TypeKind::Bitset { .. }
        | TypeKind::Valuetype { .. }
        | TypeKind::Native
        | TypeKind::AnnotationType { .. } => rust::Type::named(relative_path(ast, from, node)?),
        TypeKind::ValueBox { boxed } => rust::Type::generic(
            Path::new(&["Box"]),
            vec![type_of_at(ast, from, boxed.resolved(), depth + 1)?],
        ),
        // `fixed<P,S>` becomes the decimal's text, which is what every
        // other column does with it — C++ `std::string`, F# and OCaml and
        // D their string type — and what the default value
        // `zero_decimal(scale)` already spells. It used to emit the name
        // `Fixed`, which no crate in the tree declares: ten corpus cases
        // rendered, reported success, and failed at `rustc` with
        // `cannot find type Fixed in this scope`.
        TypeKind::Fixed { .. } => rust::Type::Primitive(rust::Primitive::String),
        // The three below have no Rust spelling. Naming them anyway put a
        // type nobody declares into a file that reports success — the
        // failure this error exists for. C++ refuses them the same way.
        TypeKind::ObjectRef { .. } => {
            return Err(MaterialiseError::NoLanguageForm {
                construct: "Object".to_string(),
                language: "Rust",
                at: handle_of(target),
            });
        }
        TypeKind::Any => {
            return Err(MaterialiseError::NoLanguageForm {
                construct: "any".to_string(),
                language: "Rust",
                at: handle_of(target),
            });
        }
        TypeKind::ValueBase => {
            return Err(MaterialiseError::NoLanguageForm {
                construct: "ValueBase".to_string(),
                language: "Rust",
                at: handle_of(target),
            });
        }
    })
}

/// The derives every materialised data type carries.
///
/// `pub(crate)`: `emit::interface_rust` gives its own synthesized
/// structures and unions the same three, for the same reason this file
/// gives them to every declared one.
pub(crate) fn data_derives() -> Vec<rust::Derive> {
    ["Debug", "Clone", "PartialEq"]
        .into_iter()
        .map(|name| rust::Derive {
            path: Path::single(name),
        })
        .collect()
}

/// A model node's name, or its identity when the node is anonymous.
///
/// An array type declared inline has no name of its own, and an error
/// that said only "the array length does not evaluate" would leave the
/// reader to find which array.
fn named(name: &str, node: zerodds_ast_core::NodeId) -> String {
    if name.is_empty() {
        format!("{node:?}")
    } else {
        name.to_string()
    }
}

/// The Rust primitive an IDL primitive becomes.
///
/// `long double` has no Rust *built-in* and is not routed here: it takes
/// `zerodds_float128::Float128`, which is a real binary128. The arm below
/// exists so the match is total and answers for a caller that wants the
/// nearest built-in; nothing in the materialiser reaches it for a
/// `long double`.
const fn rust_primitive(kind: PrimitiveKind) -> rust::Primitive {
    match kind {
        PrimitiveKind::Boolean => rust::Primitive::Bool,
        PrimitiveKind::Char => rust::Primitive::Char,
        PrimitiveKind::WChar => rust::Primitive::Char,
        PrimitiveKind::Octet | PrimitiveKind::UInt8 => rust::Primitive::U8,
        PrimitiveKind::Int8 => rust::Primitive::I8,
        PrimitiveKind::Int16 => rust::Primitive::I16,
        PrimitiveKind::UInt16 => rust::Primitive::U16,
        PrimitiveKind::Int32 => rust::Primitive::I32,
        PrimitiveKind::UInt32 => rust::Primitive::U32,
        PrimitiveKind::Int64 => rust::Primitive::I64,
        PrimitiveKind::UInt64 => rust::Primitive::U64,
        PrimitiveKind::Float32 => rust::Primitive::F32,
        PrimitiveKind::Float64 | PrimitiveKind::Float128 => rust::Primitive::F64,
    }
}

/// The type and the literal a Rust constant takes.
///
/// The type is not always the member type. `rust_type_of` answers
/// `String` for a `string`, a `wstring` and a `fixed`, and a `String` is
/// heap-allocated — `const NAME: String` does not compile, because a
/// `const` is an expression evaluated at compile time and `String::from`
/// is not one. The three become `&'static str`, which is the same bytes
/// with no allocation and converts at every use.
///
/// `long double` is refused by name. `zerodds_float128::Float128` is a
/// struct with no literal, and an `f64` would put a different number in
/// the output than the IDL wrote — which is the defect the member type's
/// own comment records.
/// `&'static str`, the type the three string-shaped constants take.
fn static_str() -> rust::Type {
    rust::Type::Ref {
        target: Box::new(rust::Type::Primitive(rust::Primitive::Str)),
        mutable: false,
        lifetime: Some(Ident::new("static")),
    }
}

fn rust_constant(
    value: &zerodds_ast::ConstValue,
    declared: rust::Type,
    at: &str,
) -> Result<(rust::Type, zerodds_lang_cst::common::Literal), MaterialiseError> {
    use zerodds_ast::ConstValue;
    use zerodds_lang_cst::common::Literal;
    let refuse = |construct: &str| MaterialiseError::NoLanguageForm {
        construct: construct.to_string(),
        language: "Rust",
        at: at.to_string(),
    };
    let is_float128 = matches!(&declared, rust::Type::Named { path, .. } if path
        .segments()
        .last()
        .is_some_and(|last| last.as_str() == "Float128"));
    Ok(match value {
        ConstValue::Boolean(flag) => (declared, Literal::Bool(*flag)),
        ConstValue::Integer(number) => (declared, Literal::int(*number)),
        // A Rust `char` is a Unicode scalar value, which is exactly what
        // both character forms carry — no width question and no cast.
        ConstValue::Char(character) => (declared, Literal::Char(*character)),
        ConstValue::WChar(point) => match constant::wchar_char(*point) {
            Some(character) => (declared, Literal::Char(character)),
            None => {
                return Err(refuse(
                    "a `wchar` constant that is not a Unicode scalar value",
                ));
            }
        },
        ConstValue::Float { mantissa, exponent } => {
            if is_float128 {
                return Err(refuse(
                    "a `long double` constant, which has no Rust literal",
                ));
            }
            // No suffix: the constant's declared type says `f32` or
            // `f64`, and a suffix that disagreed with it would be the
            // error rather than the fix.
            (
                declared,
                Literal::Float(constant::float_text(mantissa, exponent.as_deref())),
            )
        }
        ConstValue::Fixed { digits, scale } => (
            static_str(),
            Literal::Str(constant::fixed_decimal(digits, *scale)),
        ),
        ConstValue::String(text) => (static_str(), Literal::Str(text.clone())),
        ConstValue::WString(points) => (
            static_str(),
            Literal::Str(
                constant::wstring_utf8(points)
                    .ok_or_else(|| refuse("a `wstring` constant holding a lone surrogate"))?,
            ),
        ),
        // `ConstValue` is `#[non_exhaustive]`, so a form added to the
        // model reaches here. Refused by name rather than given a
        // plausible default.
        _ => return Err(refuse("a constant form this binding has no literal for")),
    })
}
