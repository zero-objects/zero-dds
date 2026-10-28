// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Annotations, from the CST into the facets. Spec §4, §5.
//!
//! Each specified annotation reaches its typed field; anything a user
//! declared with `@annotation` reaches [`Facets::annotations`]. Nothing
//! is dropped on the way, because a dropped annotation would make a
//! regenerated declaration differ from the one that was written.
//!
//! An annotation whose argument does not make sense is a diagnostic, not
//! a silently ignored one — `@id("x")` must not leave the member with
//! the id it would have had without the annotation.

use crate::lower::Lowering;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use zerodds_ast::{
    AnnotationDefNode, AnyId, AppliedAnnotation, AppliedArgument, AutoId, ConstExprNode,
    DataRepresentationMask, Extensibility, MemberIdAssignment, MemberIdOrigin, RangeConstraint,
    TopicAnnotation, TryConstruct, TypeKind, TypeNode, VerbatimDirective, VerbatimPlacement,
};
use zerodds_ast_core::{Id, PendingRef, Ref, ScopedName};
use zerodds_idl_cst::{CstKind, Span, SyntaxNode, TokenKind};
use zerodds_idl_parse::DiagnosticCode;

/// The largest member id: 28 bits. DDS-XTypes 1.3 §7.2.2.4.4.4.4.
const MAX_MEMBER_ID: u32 = 0x0FFF_FFFF;

/// Whether an annotation node was written in the suffix form.
///
/// Its first token is the marker, and the two forms have different ones.
fn is_suffix_form(node: &Arc<SyntaxNode>) -> bool {
    node.syntax_tokens()
        .first()
        .is_some_and(|(_, token)| token.kind() == TokenKind::SlashSlashAt)
}

/// One annotation as the source wrote it.
#[derive(Clone)]
pub(crate) struct Applied {
    /// Name without the `@`.
    pub(crate) name: String,
    /// Arguments in written order.
    pub(crate) arguments: Vec<Argument>,
    /// Byte range of the whole annotation.
    pub(crate) span: Span,
}

/// One argument of an annotation.
#[derive(Clone)]
pub(crate) struct Argument {
    /// The parameter name, for the `k = v` form.
    pub(crate) name: Option<String>,
    /// The argument as a model expression.
    pub(crate) value: Option<Id<ConstExprNode>>,
    /// Identifiers written in the argument.
    ///
    /// `@autoid(HASH)` and `@data_representation(XCDR1 | XCDR2)` name
    /// enumerators of the annotation's own type, which are not model
    /// constants; the written identifiers are what decides them.
    pub(crate) identifiers: Vec<String>,
    /// The string literal, when the argument is one.
    pub(crate) text: Option<String>,
    /// Byte range of the argument.
    pub(crate) span: Span,
}

/// Which slot of a parameterised type an annotation was written on.
///
/// The positions DDS-XTypes 1.3 §7.3.1.2.1.12 names beside the member
/// itself: "the union discriminator, the elements of arrays and
/// sequences, and the key and/or values of map types".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UseSite {
    /// A sequence or array element, or a map's value.
    Element,
    /// A map's key.
    MapKey,
    /// A union's discriminator.
    Discriminator,
}

/// Where an annotation was written, in the terms of Table 21.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Place {
    StructMember,
    UnionMember,
    Type(PlaceKind),
    Module,
    Enumerator,
    BitValue,
    Bitfield,
    Other,
}

/// What kind of type declaration an annotation was written on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlaceKind {
    /// A structure, a union or a valuetype.
    Aggregated,
    /// An enumeration or a bitmask.
    Enumerated,
    /// A bitset.
    Bitset,
    /// A `typedef`.
    Alias,
    /// Any other type declaration.
    Other,
}

impl UseSite {
    /// What to call it in a diagnostic.
    const fn describe(self) -> &'static str {
        match self {
            Self::Element => "a collection element",
            Self::MapKey => "a map key",
            Self::Discriminator => "a union discriminator",
        }
    }
}

impl Argument {
    /// The single identifier the argument names, if it names exactly one.
    fn identifier(&self) -> Option<&str> {
        match self.identifiers.as_slice() {
            [only] => Some(only.as_str()),
            _ => None,
        }
    }
}

/// Whether the built-in annotation of this name takes its argument from
/// an enumerated type of its own rather than from the file's constants.
///
/// `@annotation autoid { AutoidKind value default HASH; }` and the three
/// like it declare the enumeration alongside the annotation — IDL 4.2
/// §8.3.1.2, §8.3.4.1, §8.3.5.2 and DDS-XTypes 1.3 §7.6.3.1.1 — so
/// `HASH`, `MUTABLE`, `TRIM` and `XCDR2` are names the file itself never
/// declares. They are read from the argument's written identifiers.
fn enumerated_parameter(annotation: &str) -> bool {
    matches!(
        annotation,
        // `@verbatim`'s `placement` is one: IDL 4.2 §8.3.5.1 declares it
        // as `PlacementKind placement`, and `BEFORE_DECLARATION` is an
        // enumerator of that type rather than a name of the file.
        // `verbatim_annotation` reads every one of its parameters out of
        // `Argument::text` and `Argument::identifier`, so nothing is
        // lost by not lowering the expression.
        // `Autoid` is DDS-RPC 1.0 §7.5.1.2.1.2's capitalised annotation;
        // `@Autoid(HASH | SEQUENTIAL)` names an enumerator of its own
        // type, exactly as the DDS-XTypes `@autoid` does.
        "autoid"
            | "Autoid"
            | "extensibility"
            | "try_construct"
            | "data_representation"
            | "verbatim"
    )
}

impl Lowering<'_> {
    /// Read every annotation written directly under a node.
    pub(crate) fn annotations_of(&mut self, node: &Arc<SyntaxNode>) -> Vec<Applied> {
        node.children()
            .into_iter()
            .filter(|child| child.kind() == CstKind::Annotation)
            .filter_map(|annotation| self.annotation(&annotation))
            .collect()
    }

    /// Read only the annotations written in the suffix form.
    ///
    /// A declaration node holds its prefix annotations nowhere — those
    /// sit in the wrapper node the parser puts around them — but it does
    /// hold the ones written after its closing `;`, and it may hold one
    /// that belongs to something else: `union U switch (@key long)` puts
    /// the discriminator's annotation directly under the `UnionDecl`.
    /// The marker token is what tells the two apart. DDS-XTypes 1.3
    /// §7.3.1.2.5, Table 24.
    pub(crate) fn suffix_annotations_of(&mut self, node: &Arc<SyntaxNode>) -> Vec<Applied> {
        node.children()
            .into_iter()
            .filter(|child| child.kind() == CstKind::Annotation && is_suffix_form(child))
            .filter_map(|annotation| self.annotation(&annotation))
            .collect()
    }

    /// One `@…` node, read as an [`Applied`].
    ///
    /// The same reading [`Self::annotations_of`] does, for the callers
    /// that have the annotation node itself rather than its parent —
    /// the template-argument and discriminator positions, whose
    /// annotations are siblings of a type specification and not of a
    /// declaration.
    pub(crate) fn applied_annotation(&mut self, node: &Arc<SyntaxNode>) -> Option<Applied> {
        self.annotation(node)
    }

    fn annotation(&mut self, node: &Arc<SyntaxNode>) -> Option<Applied> {
        // The name is a `<scoped_name>` — §8.2, rule (218) — and every
        // segment of it is an identifier or a keyword, because
        // `@default` and `@public` collide with keywords by design.
        //
        // The **last** segment is the name that resolves: every builtin
        // is matched by its simple name, and `@ann::mark` and `@::key`
        // name `mark` and `key`. Taking the first token after `@`
        // answered `ann` for the first of those and `::` for the second.
        let name = node
            .syntax_tokens()
            .into_iter()
            .take_while(|(_, token)| token.kind() != TokenKind::LParen)
            .filter(|(_, token)| {
                !matches!(
                    token.kind(),
                    // `//@key` and `@key` name the same annotation:
                    // DDS-XTypes 1.3 §7.3.1.2.5, Table 24 makes the two
                    // markers equivalent, so both are skipped on the way
                    // to the name.
                    TokenKind::At | TokenKind::SlashSlashAt | TokenKind::ColonColon
                )
            })
            .last()
            .map(|(_, token)| zerodds_idl_cst::identifier_name(token.text()).to_string())?;

        // DDS-XTypes 1.3 §7.3.1.2.5: annotation names are "not
        // case-sensitive", so `@Key` and `@KEY` are `@key`. A name the
        // file declared itself keeps its exact spelling and wins: a file
        // that declares `@annotation Key { … }` means that one.
        let name = if zerodds_ast::node::annotation::BUILTIN_ANNOTATIONS.contains(&name.as_str())
            || matches!(self.scopes.lookup(&name), Some(AnyId::AnnotationDef(_, _)))
        {
            name
        } else {
            let lowered = name.to_ascii_lowercase();
            if zerodds_ast::node::annotation::BUILTIN_ANNOTATIONS.contains(&lowered.as_str()) {
                lowered
            } else {
                name
            }
        };

        // DDS-XTypes 1.0 Beta 1, which DDS-PSM-Cxx 1.0 (§8.1.5) refers to, calls `@Shared` what
        // XTypes 1.3 calls `@external`; the old name lowers to the new one unless the file
        // declared an annotation of that name itself.
        let name = if name.eq_ignore_ascii_case("shared")
            && !matches!(self.scopes.lookup(&name), Some(AnyId::AnnotationDef(_, _)))
        {
            "external".to_string()
        } else {
            name
        };

        // IDL 4.2 §7.4.15.4.2: "in case an unknown annotation is
        // encountered, it should be ignored by the IDL-processing
        // tools." An unknown one is one this model has no builtin for
        // and the file did not declare, and *ignored* has to reach its
        // arguments too — see `Self::argument`.
        let unknown = !zerodds_ast::node::annotation::BUILTIN_ANNOTATIONS.contains(&name.as_str())
            && !matches!(self.scopes.lookup(&name), Some(AnyId::AnnotationDef(_, _)));

        let arguments = node
            .children()
            .into_iter()
            .find(|child| child.kind() == CstKind::AnnotationArguments)
            .map(|list| {
                list.children()
                    .into_iter()
                    .filter(|child| child.kind() == CstKind::AnnotationArgument)
                    .map(|argument| {
                        self.argument(&argument, enumerated_parameter(&name) || unknown)
                    })
                    .collect()
            })
            .unwrap_or_default();

        Some(Applied {
            name,
            arguments,
            span: node.span(),
        })
    }

    /// One argument of an annotation application.
    ///
    /// `names_are_not_of_this_file` says that an identifier written here
    /// does not name anything the file declares, so lowering it as an
    /// expression would plant a reference the resolve pass then breaks.
    /// Two cases share it, and they share a cause:
    ///
    /// - an **enumerated parameter** of a builtin — `@autoid(HASH)`,
    ///   `@extensibility(FINAL)` — whose argument is an enumerator of
    ///   the annotation's own type and is in no scope of the file;
    /// - an **unknown annotation**, which IDL 4.2 §7.4.15.4.2 says to
    ///   ignore. `@RTI_topic_kind(NO_KEY)` and
    ///   `@allowed_data_representation(XCDR2)` are the form vendors
    ///   write, and resolving `NO_KEY` while ignoring the annotation it
    ///   belongs to broke every file that used one.
    ///
    /// What was written is not lost: `identifiers` and `text` carry it,
    /// and the formatter renders the token stream rather than the model.
    fn argument(&mut self, node: &Arc<SyntaxNode>, names_are_not_of_this_file: bool) -> Argument {
        // `k = v` writes the parameter name as a bare identifier before
        // the expression; the expression's own names are inside it.
        let name = node
            .syntax_tokens()
            .iter()
            .position(|(_, token)| token.kind() == TokenKind::Equals)
            .and_then(|_| {
                node.syntax_tokens()
                    .first()
                    .filter(|(_, token)| token.kind() == TokenKind::Ident)
                    .map(|(_, token)| zerodds_idl_cst::identifier_name(token.text()).to_string())
            });

        // The identifiers and the literal live inside the argument's
        // expression, not directly under the argument node.
        let identifiers: Vec<String> = node
            .descendants()
            .into_iter()
            .filter(|child| matches!(child.kind(), CstKind::NameExpr | CstKind::ScopedName))
            .flat_map(|expr| {
                expr.syntax_tokens()
                    .into_iter()
                    .filter(|(_, token)| token.kind() == TokenKind::Ident)
                    .map(|(_, token)| zerodds_idl_cst::identifier_name(token.text()).to_string())
                    .collect::<Vec<String>>()
            })
            .collect();

        // The whole literal, adjacent pieces joined (§7.2.6.3): the first
        // piece alone was `@verbatim(text = "a" "b")` reaching the
        // generator as `a`.
        let text = crate::lower::joined_string_literals(
            node.descendants()
                .into_iter()
                .flat_map(|child| child.syntax_tokens()),
            TokenKind::StringLiteral,
        )
        .into_iter()
        .next();

        // A literal argument is lowered either way — `@nosuchthing(v = 1)`
        // keeps its 1. It is only an *identifier* that has nothing in
        // this file to resolve to.
        let value = if names_are_not_of_this_file && !identifiers.is_empty() {
            None
        } else {
            self.first_expr_child(node)
        };

        Argument {
            name,
            value,
            identifiers,
            text,
            span: node.span(),
        }
    }

    /// Apply every annotation of a declaration to the node it produced.
    pub(crate) fn apply_annotations(&mut self, annotations: &[Applied], target: AnyId) {
        self.reject_two_member_ids(annotations);
        for applied in annotations {
            self.apply_one(applied, target);
        }
    }

    /// "To assign the member ID explicitly the member shall have either
    /// the @id or the @hashid annotation. At most one of these two
    /// annotations may appear on a member. Otherwise it shall be
    /// considered an error." DDS-XTypes 1.3 §7.3.1.2.1.1.
    ///
    /// Both are still applied after the diagnostic, so the member keeps
    /// whichever id the written order gives it and the rest of the file
    /// lowers; the error is what stops the type from being accepted.
    /// Measured: Cyclone DDS and Fast DDS both refuse the pair, RTI
    /// Connext accepts it and lets `@hashid` win.
    fn reject_two_member_ids(&mut self, annotations: &[Applied]) {
        let named = |wanted: &str| annotations.iter().find(|a| a.name == wanted);
        let (Some(id), Some(hashid)) = (named("id"), named("hashid")) else {
            return;
        };
        let second = if id.span.start > hashid.span.start {
            id
        } else {
            hashid
        };
        self.annotation_error(
            second,
            "`@id` and `@hashid` cannot both appear on one member",
            "DDS-XTypes 1.3 §7.3.1.2.1.1",
        );
    }

    /// The element of the type system an annotation was written on, in
    /// the terms of Table 21 of DDS-XTypes 1.3.
    fn place_of(&self, target: AnyId) -> Place {
        match target {
            AnyId::Member(id, _) => {
                let union = self.ast.member(id).is_ok_and(|member| {
                    matches!(
                        member.parent,
                        AnyId::Type(parent, _)
                            if self
                                .ast
                                .type_(parent)
                                .is_ok_and(|t| matches!(t.kind, TypeKind::Union { .. }))
                    )
                });
                if union {
                    Place::UnionMember
                } else {
                    Place::StructMember
                }
            }
            AnyId::Case(..) => Place::UnionMember,
            AnyId::Type(id, _) => match self.ast.type_(id).map(|t| &t.kind) {
                Ok(TypeKind::Enumeration { .. }) => Place::Type(PlaceKind::Enumerated),
                Ok(TypeKind::Bitmask { .. }) => Place::Type(PlaceKind::Enumerated),
                Ok(TypeKind::Bitset { .. }) => Place::Type(PlaceKind::Bitset),
                Ok(TypeKind::Alias { .. }) => Place::Type(PlaceKind::Alias),
                Ok(
                    TypeKind::Structure { .. }
                    | TypeKind::Valuetype { .. }
                    | TypeKind::Union { .. },
                ) => Place::Type(PlaceKind::Aggregated),
                _ => Place::Type(PlaceKind::Other),
            },
            AnyId::Module(..) => Place::Module,
            AnyId::Enumerator(..) => Place::Enumerator,
            AnyId::BitValue(..) => Place::BitValue,
            AnyId::Bitfield(..) => Place::Bitfield,
            _ => Place::Other,
        }
    }

    /// Whether `target` is a structure declaration (not a union, a
    /// valuetype or an alias of one).
    fn is_structure(&self, target: AnyId) -> bool {
        matches!(
            target,
            AnyId::Type(id, _)
                if self
                    .ast
                    .type_(id)
                    .is_ok_and(|t| matches!(t.kind, TypeKind::Structure { .. }))
        )
    }

    /// Whether Table 21 allows the annotation where it was written, and
    /// the list of places it names when it does not.
    ///
    /// DDS-XTypes 1.3 §7.3.1.2.2: "The application of the annotations
    /// listed above is restricted to the type elements specified in
    /// Table 21." An annotation on another element used to be set on
    /// whatever node it was written on — `@bit_bound` on a structure,
    /// `@position` on a member, `@autoid` on an enumeration — and read by
    /// nothing, which is a declaration that says something the model
    /// does not hold.
    ///
    /// A place this table says nothing about (an operation, a parameter,
    /// a constant) is not judged.
    fn misplaced(&self, name: &str, target: AnyId) -> Option<&'static str> {
        // DDS-RPC 1.0 §7.5.1.2.1.1: "The @Choice annotation is allowed on
        // structures only." An interface, an operation or a member is not
        // a place this table leaves unjudged for it.
        if name == "Choice" {
            return if self.is_structure(target) {
                None
            } else {
                Some("structures")
            };
        }
        // §7.5.1.2.1.2: "The @Autoid annotation shall be allowed on
        // structures and data members."
        if name == "Autoid"
            && !(self.is_structure(target) || matches!(self.place_of(target), Place::StructMember))
        {
            return Some("structures and structure members");
        }
        let place = self.place_of(target);
        if matches!(place, Place::Other) {
            return None;
        }
        let (allowed, listed): (bool, &'static str) = match name {
            "optional" | "must_understand" | "non_serialized" => (
                // `@optional` on a union member is refused by name, with
                // its own clause, by `union_member_rules`.
                matches!(place, Place::StructMember)
                    || (name == "optional" && matches!(place, Place::UnionMember)),
                "structure members",
            ),
            "id" | "hashid" => (
                matches!(place, Place::StructMember | Place::UnionMember),
                "structure members and union members",
            ),
            "key" => (
                matches!(place, Place::StructMember | Place::UnionMember),
                "structure members and union discriminators",
            ),
            // Table 21 gives it to enumerated and bitmask types. A bitset
            // keeps it for the width of its anchor and a `typedef` of an
            // integer keeps it as the width of the alias, which the
            // language adapters read (`@bit_bound(40) typedef long long
            // Counter40;`), so neither is a declaration that says
            // something no consumer holds.
            "bit_bound" => (
                matches!(
                    place,
                    Place::Type(PlaceKind::Enumerated | PlaceKind::Bitset | PlaceKind::Alias)
                ),
                "enumerated types and bitmask types",
            ),
            "extensibility" | "mutable" | "appendable" | "final" | "nested" => {
                (matches!(place, Place::Type(_)), "type declarations")
            }
            "default_literal" | "value" => {
                (matches!(place, Place::Enumerator), "enumerated literals")
            }
            "position" => (matches!(place, Place::BitValue), "bitmask values"),
            "autoid" => (
                matches!(place, Place::Module | Place::Type(PlaceKind::Aggregated)),
                "module, structure and union declarations",
            ),
            _ => return None,
        };
        if allowed { None } else { Some(listed) }
    }

    #[allow(clippy::too_many_lines)]
    fn apply_one(&mut self, applied: &Applied, target: AnyId) {
        // §7.3.1.2.1.8: the extensibility annotations "may be applied to
        // the definitions of aggregated types. It shall be considered an
        // error for it to be applied to the same type multiple times."
        // `@final @mutable struct S`, `@final @extensibility(FINAL) struct
        // S` and the like used to take the last one written.
        if matches!(
            applied.name.as_str(),
            "final" | "appendable" | "mutable" | "extensibility"
        ) && self
            .ast
            .facets(target)
            .and_then(|facets| facets.xtypes.as_ref())
            .is_some_and(|x| x.extensibility.is_some())
        {
            self.annotation_error(
                applied,
                "the extensibility of a type is given once; `@final`, `@appendable`, \
                 `@mutable` and `@extensibility` are applied to this one more than once",
                "DDS-XTypes 1.3 §7.3.1.2.1.8",
            );
            return;
        }
        if let Some(listed) = self.misplaced(&applied.name, target) {
            self.annotation_error(
                applied,
                &alloc::format!("`@{}` applies to {listed}", applied.name),
                "DDS-XTypes 1.3 §7.3.1.2.2, Table 21",
            );
            return;
        }
        // `annotation` has already matched a builtin's name without
        // regard to case (§7.3.1.2.5), so `applied.name` is the
        // builtin's own spelling here.
        match applied.name.as_str() {
            // The boolean annotations of §7.3.1.2.1 take `boolean value
            // default TRUE`: `@key` is `@key(TRUE)` and `@key(FALSE)` is
            // a member that is not a key.
            "key" => {
                let value = self.flag_argument(applied);
                self.set_xtypes(target, |x| x.is_key = value);
            }
            "optional" => {
                let value = self.flag_argument(applied);
                self.set_xtypes(target, |x| x.is_optional = value);
            }
            "external" => {
                let value = self.flag_argument(applied);
                self.set_xtypes(target, |x| x.is_external = value);
            }
            "must_understand" => {
                let value = self.flag_argument(applied);
                self.set_xtypes(target, |x| x.is_must_understand = value);
            }
            "non_serialized" => {
                let value = self.flag_argument(applied);
                self.set_xtypes(target, |x| x.is_non_serialized = value);
            }
            "nested" => {
                let value = self.flag_argument(applied);
                self.set_xtypes(target, |x| {
                    x.is_nested = value;
                    x.nested_explicit = true;
                });
            }
            // §7.3.1.2.1.7. On a module, and the default for a type in
            // it that carries no `@nested` of its own. `Some(false)` and
            // `None` are different things — see
            // `XTypesFacet::default_nested` and `Ast::effective_nested`.
            "default_nested" => {
                let value = self.flag_argument(applied);
                self.set_xtypes(target, |x| x.default_nested = Some(value));
            }
            "default_literal" => self.set_xtypes(target, |x| x.is_default_literal = true),
            "ignore_literal_names" => {
                let value = self.flag_argument(applied);
                self.set_xtypes(target, |x| x.ignore_literal_names = value);
            }
            "final" => self.set_extensibility(target, Extensibility::Final),
            "appendable" => self.set_extensibility(target, Extensibility::Appendable),
            "mutable" => self.set_extensibility(target, Extensibility::Mutable),
            "extensibility" => self.extensibility_argument(applied, target),
            "id" => self.id_annotation(applied, target),
            "hashid" => self.hashid_annotation(applied, target),
            "autoid" => self.autoid_annotation(applied, target),
            "value" | "position" => self.value_annotation(applied, target),
            "bit_bound" => self.bit_bound_annotation(applied, target),
            "try_construct" => self.try_construct_annotation(applied, target),
            "default" => {
                if let Some(value) = applied.arguments.first().and_then(|a| a.value) {
                    self.set_xtypes(target, |x| x.default_value = Some(value));
                }
            }
            "min" | "max" | "range" => self.range_annotation(applied, target),
            "unit" => {
                if let Some(unit) = applied.arguments.first().and_then(|a| a.text.clone()) {
                    self.set_xtypes(target, |x| x.unit = Some(unit.clone()));
                } else {
                    self.annotation_error(
                        applied,
                        "`@unit` takes one string",
                        "DDS-XTypes 1.3 Annex B",
                    );
                }
            }
            "verbatim" => self.verbatim_annotation(applied, target),
            "topic" => self.topic_annotation(applied, target),
            "data_representation" => self.data_representation_annotation(applied, target),
            // `@DDSService` is how DDS-RPC 1.0 writes the annotation in its
            // own examples (§7.3.1.3, §7.4.2.2) and defines it for Java
            // (§7.3.2); it marks the same thing as IDL 4.2's `@service`.
            "service" | "DDSService" => self.service_annotation(applied, target),
            // DDS-RPC 1.0 §7.5.1.2.1.2: `@Autoid` assigns the member ids
            // of a structure by the HASH of the member names, and a
            // member annotated alone takes the HASH of its own name. The
            // application is kept whole as well, like every annotation
            // of the specification the model has no field for.
            "Autoid" => {
                self.dds_rpc_autoid(applied, target);
                self.apply_user_annotation(applied, target);
            }
            "ami" => self.set_corba(target, |c| c.is_ami = true),
            // `@oneway` is the operation's own flag, already set from the
            // keyword form; the annotation form sets the same field.
            "oneway" => self.set_oneway(target),
            _ => self.apply_user_annotation(applied, target),
        }
    }

    /// `@service`, with whatever it was given.
    ///
    /// This used to set the flag and drop the arguments, which made
    /// `@service` and `@service(name = "Calculator")` the same model.
    /// IDL 4.2 §8.3.4 declares one member on the annotation and DDS-RPC
    /// 1.0 §7.3 reads another from it, so the arguments are carried
    /// through unread and the consumer decides.
    fn service_annotation(&mut self, applied: &Applied, target: AnyId) {
        let arguments: Vec<AppliedArgument> = applied
            .arguments
            .iter()
            .filter_map(|argument| {
                argument.value.map(|value| AppliedArgument {
                    name: argument.name.clone(),
                    value,
                })
            })
            .collect();
        self.set_corba(target, |c| {
            c.is_service = true;
            c.service_arguments = arguments;
        });
    }

    /// A user-declared annotation, kept whole.
    fn apply_user_annotation(&mut self, applied: &Applied, target: AnyId) {
        let annotation = self.user_annotation(applied);
        if let Some(facets) = self.ast.facets_mut(target) {
            facets.annotations.push(annotation);
        }
    }

    /// The model's form of one application of a user-declared annotation,
    /// checked against its declaration when that is already known.
    fn user_annotation(&mut self, applied: &Applied) -> AppliedAnnotation {
        // The built-in declarations are looked up by their exact name: the
        // scope lookup is case-insensitive (IDL 4.2 §7.5), and DDS-RPC 1.0's
        // `@Mutable` (§7.5.1.2.1.4) and DDS-XTypes' `@mutable` differ in
        // case only, so the lookup finds neither.
        let definition = match self.scopes.lookup(&applied.name) {
            Some(AnyId::AnnotationDef(id, _)) => Ref::Resolved(id),
            _ => match self.ast.builtin_annotation(&applied.name) {
                Some(id) => Ref::Resolved(id),
                None => Ref::Pending(PendingRef::ScopedName(ScopedName::absolute(alloc::vec![
                    applied.name.clone()
                ]))),
            },
        };
        let arguments: Vec<AppliedArgument> = applied
            .arguments
            .iter()
            .filter_map(|argument| {
                argument.value.map(|value| AppliedArgument {
                    name: argument.name.clone(),
                    value,
                })
            })
            .collect();
        // Only a declaration the file made is held to its members: a built-in
        // one has the implicit `value` member of the short form and no default.
        if let (Ref::Resolved(id), Some(AnyId::AnnotationDef(..))) =
            (&definition, self.scopes.lookup(&applied.name))
        {
            self.check_annotation_application(applied, *id);
        }
        AppliedAnnotation {
            name: applied.name.clone(),
            definition,
            arguments,
        }
    }

    /// One application of a **declared** annotation, against its
    /// declaration. IDL 4.2 §7.4.15.4.2.
    ///
    /// Four sentences of that clause, none of which held:
    ///
    /// > Members with no default value must be given a value
    ///
    /// > A constant expression, whose type must be compatible with the
    /// > member's declaration
    ///
    /// > In case there is only one member, the annotation application may
    /// > be shortened to the name of the annotation prefixed with @ and
    /// > followed with the constant value of that unique member
    ///
    /// > malformed definitions shall be treated as an error
    ///
    /// **Declared annotations only, and that boundary is the clause's
    /// own.** The same paragraph says an *unknown* annotation "should be
    /// ignored by the IDL-processing tools", so none of this may reach
    /// `@RTI_topic_kind(NO_KEY)` or any other annotation the file did not
    /// declare. The caller only reaches here with a resolved definition,
    /// which is exactly that line.
    fn check_annotation_application(&mut self, applied: &Applied, id: Id<AnnotationDefNode>) {
        let Ok(definition) = self.ast.annotation_def(id) else {
            return;
        };
        let members: Vec<(String, bool, Option<Ref<TypeNode>>)> = definition
            .members
            .iter()
            .filter_map(|member| self.ast.annotation_member(*member).ok())
            .map(|member| {
                (
                    member.name.clone(),
                    member.default.is_some(),
                    Some(member.type_.clone()),
                )
            })
            .collect();
        if members.is_empty() {
            return;
        }
        let name = applied.name.clone();

        // The short form. "In case there is only one member" — with two
        // or more, a positional argument names nothing.
        let positional = applied
            .arguments
            .iter()
            .filter(|a| a.name.is_none())
            .count();
        if positional > 0 && members.len() > 1 {
            self.annotation_error(
                applied,
                &alloc::format!(
                    "`@{name}` declares {} members, so its arguments have to be named; the \
                     short form `@{name}(value)` is only for an annotation with one member",
                    members.len()
                ),
                "OMG IDL 4.2 §7.4.15.4.2",
            );
            return;
        }

        // A named argument that names nothing is the "malformed
        // definition" the clause ends on.
        for argument in &applied.arguments {
            let Some(written) = argument.name.as_deref() else {
                continue;
            };
            // "As with any IDL identifier, the name of an annotation and
            // its members are not case-sensitive." (§7.3.1.2.5)
            if !members
                .iter()
                .any(|(member, _, _)| member.eq_ignore_ascii_case(written))
            {
                let known: Vec<&str> = members.iter().map(|(m, _, _)| m.as_str()).collect();
                self.annotation_error(
                    applied,
                    &alloc::format!(
                        "`@{name}` has no member named `{written}`; it declares {}",
                        known.join(", ")
                    ),
                    "OMG IDL 4.2 §7.4.15.4.2",
                );
            }
        }

        // A member the application omits is not an error. DDS-XTypes
        // 1.3 §7.3.1.2.5: "Any member of the annotation interface may be
        // omitted when the annotation is applied. If a value for a given
        // member is omitted, and that member has a defined default value,
        // it will take that value. If an omitted member does not have a
        // specified default, it will take the default value specified for
        // its type in Clause 7.2.2.4.4.4.7." IDL 4.2 §7.4.15.4.2 says
        // "Members with no default value must be given a value"; this
        // front end reads the specification it implements. The default of
        // the member's type is what a consumer of the application
        // supplies, as it supplies a declared default.

        // "A constant expression, whose type must be compatible with the
        // member's declaration."
        for argument in &applied.arguments {
            let member = if argument.name.is_none() && members.len() == 1 {
                members.first()
            } else {
                let written = argument.name.as_deref();
                members.iter().find(|(member, _, _)| {
                    written.is_some_and(|written| member.eq_ignore_ascii_case(written))
                })
            };
            let Some((member_name, _, Some(declared))) = member else {
                continue;
            };
            let Some(value) = argument.value else {
                continue;
            };
            let declared = declared.clone();
            let member_name = member_name.clone();
            if let Some(complaint) = self.value_disagrees_with(&declared, value) {
                self.annotation_error(
                    applied,
                    &alloc::format!("`@{name}`'s `{member_name}` {complaint}"),
                    "OMG IDL 4.2 §7.4.15.4.2",
                );
            }
        }
    }

    // -- the specified annotations, one rule each ------------------------

    fn id_annotation(&mut self, applied: &Applied, target: AnyId) {
        let Some(value) = self.integral_argument(applied) else {
            return;
        };
        // §7.2.2.4.4.4.4: "Member IDs are unsigned and have a range that
        // can be represented in 28 bits: from zero to 268,435,455
        // (0x0FFFFFFF)."
        let Some(effective) = u32::try_from(value).ok().filter(|id| *id <= MAX_MEMBER_ID) else {
            self.annotation_error(
                applied,
                "a member id is between 0 and 0x0FFFFFFF, the range 28 bits represent",
                "DDS-XTypes 1.3 §7.2.2.4.4.4.4",
            );
            return;
        };
        self.set_xtypes(target, |x| {
            x.member_id = Some(MemberIdAssignment {
                effective,
                origin: MemberIdOrigin::Declared,
            });
        });
    }

    /// `@hashid` or `@hashid("name")`.
    ///
    /// The id itself is the hash, which the completion pass computes;
    /// what is recorded here is which name to hash. Spec §5.
    ///
    /// The empty string is not a name to hash: "If the annotation is
    /// used without any parameter or with the empty string as a
    /// parameter, then the Member ID shall be computed from the member
    /// name" — DDS-XTypes 1.3 §7.3.1.2.1.1. Hashing `""` instead is a
    /// measured divergence of Cyclone DDS, which takes any parameter it
    /// was given; RTI Connext and Fast DDS fall back to the member name.
    fn hashid_annotation(&mut self, applied: &Applied, target: AnyId) {
        let source_name = applied
            .arguments
            .first()
            .and_then(|argument| argument.text.clone())
            .filter(|written| !written.is_empty())
            .unwrap_or_else(|| self.name_of(target).unwrap_or_default());
        // The hash is computed here rather than left at a placeholder:
        // a member id of 0 waiting for the completion pass collides with
        // the first member of every aggregate, and the model rejects the
        // commit before the real value ever arrives.
        let effective = crate::complete::hashed_member_id(&source_name);
        self.set_xtypes(target, |x| {
            x.member_id = Some(MemberIdAssignment {
                effective,
                origin: MemberIdOrigin::Hashed {
                    source_name: source_name.clone(),
                },
            });
        });
    }

    fn autoid_annotation(&mut self, applied: &Applied, target: AnyId) {
        let policy = match applied
            .arguments
            .first()
            .and_then(Argument::identifier)
            .unwrap_or("HASH")
        {
            "HASH" => AutoId::Hash,
            "SEQUENTIAL" => AutoId::Sequential,
            other => {
                self.annotation_error(
                    applied,
                    &alloc::format!("`@autoid` takes HASH or SEQUENTIAL, not `{other}`"),
                    "DDS-XTypes 1.3 §7.3.1.2.1.1",
                );
                return;
            }
        };
        self.set_xtypes(target, |x| x.autoid = Some(policy));
    }

    /// DDS-RPC 1.0 §7.5.1.2.1.2 `@Autoid`: "the member-id of each member
    /// shall be computed using the HASH algorithm (defined in 7.5.1.1.2
    /// The Hashing Algorithm) applied to the name of the member."
    ///
    /// On a structure it is the policy `@autoid(HASH)` sets, which the
    /// completion pass turns into the ids; the argument `SEQUENTIAL` is
    /// accepted for the same reason `@autoid` accepts it. On a member it
    /// is `@hashid` of the member's own name.
    fn dds_rpc_autoid(&mut self, applied: &Applied, target: AnyId) {
        match target {
            AnyId::Member(id, _) => {
                let Ok(member) = self.ast.member(id) else {
                    return;
                };
                let name = member.name.clone();
                self.set_xtypes(target, |x| {
                    x.member_id = Some(zerodds_ast::MemberIdAssignment {
                        effective: 0,
                        origin: MemberIdOrigin::Hashed { source_name: name },
                    });
                });
            }
            _ => self.autoid_annotation(applied, target),
        }
    }

    /// `@value(n)` on an enumerator, `@position(k)` on a bit.
    ///
    /// Both write their number on the part itself rather than on a
    /// facet, so the model's own accessor answers without a facet lookup.
    fn value_annotation(&mut self, applied: &Applied, target: AnyId) {
        match target {
            AnyId::Enumerator(id, _) => {
                // The enumerator keeps the expression as written; the
                // number is what evaluating it gives, and a formatter
                // still has `@value(1 << 3)` rather than `8`.
                let Some(expr) = applied.arguments.first().and_then(|a| a.value) else {
                    self.annotation_error(
                        applied,
                        "`@value` needs a constant expression",
                        "OMG IDL 4.2 §7.4.13",
                    );
                    return;
                };
                if self.ast.eval_integral(expr).is_err() {
                    self.annotation_error(
                        applied,
                        "`@value` needs an integer constant",
                        "OMG IDL 4.2 §7.4.13",
                    );
                    return;
                }
                if let Ok(enumerator) = self.ast.enumerator_mut(id) {
                    enumerator.value = Some(expr);
                }
            }
            AnyId::BitValue(id, _) => {
                let Some(value) = self.integral_argument(applied) else {
                    return;
                };
                let Ok(position) = u16::try_from(value) else {
                    self.annotation_error(
                        applied,
                        "a bit position must fit in 16 bits",
                        "DDS-XTypes 1.3 §7.3.1.2.1.10",
                    );
                    return;
                };
                if let Ok(bit) = self.ast.bit_value_mut(id) {
                    bit.position = Some(position);
                }
            }
            _ => self.annotation_error(
                applied,
                "`@value` belongs on an enumerator and `@position` on a bit",
                "DDS-XTypes 1.3 §7.3.1.2.1.10",
            ),
        }
    }

    fn bit_bound_annotation(&mut self, applied: &Applied, target: AnyId) {
        let Some(value) = self.integral_argument(applied) else {
            return;
        };
        let Ok(bound) = u16::try_from(value) else {
            self.annotation_error(
                applied,
                "a bit bound must fit in 16 bits",
                "DDS-XTypes 1.3 §7.3.1.2.1.9",
            );
            return;
        };
        self.set_xtypes(target, |x| x.bit_bound = Some(bound));
    }

    fn try_construct_annotation(&mut self, applied: &Applied, target: AnyId) {
        let Some(behaviour) = self.try_construct_behaviour(applied) else {
            return;
        };
        self.set_xtypes(target, |x| x.try_construct = Some(behaviour));
    }

    /// The behaviour one `@try_construct` names.
    ///
    /// A bare `@try_construct` is **not** an error and is **not**
    /// `DISCARD`. §7.3.1.2.1.12 declares the annotation as
    /// `TryConstructFailAction value default USE_DEFAULT` and states it
    /// twice more in prose: "If the annotation is specified without a
    /// value, or if the value is set to USE_DEFAULT, then the behavior
    /// is set to DEFAULT", and Example 1 writes `@try_construct
    /// string<5> member;` as the equivalent of the USE_DEFAULT form.
    /// Reading it as an error refused a declaration the clause spells
    /// out; reading it as DISCARD — the *absent*-annotation default —
    /// would have silently dropped samples the author asked to keep.
    /// Measured: OpenDDS constructs the empty string for it. See
    /// `crates/ast-layout/tests/fixtures/try_construct_behaviour.md`.
    fn try_construct_behaviour(&mut self, applied: &Applied) -> Option<TryConstruct> {
        match applied.arguments.first() {
            None => Some(TryConstruct::BARE),
            Some(argument) => match argument.identifier().and_then(TryConstruct::parse) {
                Some(behaviour) => Some(behaviour),
                None => {
                    self.annotation_error(
                        applied,
                        &alloc::format!(
                            "`@try_construct` takes DISCARD, USE_DEFAULT or TRIM, not `{}`",
                            argument.identifier().unwrap_or("that")
                        ),
                        "DDS-XTypes 1.3 §7.3.1.2.1.12",
                    );
                    None
                }
            },
        }
    }

    /// Apply the annotations written in front of a type *use* — a
    /// collection's element, a map's key, a union's discriminator, and
    /// after an array declarator's name.
    ///
    /// Three built-in annotations have a meaning there: `@try_construct`
    /// (DDS-XTypes 1.3 §7.3.1.2.1.12), `@external` on a collection's
    /// element or key (§7.3.1.2.1.4) and `@key` on a discriminator
    /// (Table 21). A declared annotation is kept whole, on the collection
    /// or the union (§7.2.2.6, §7.3.1.2.5). Any other built-in is
    /// reported rather than dropped: an annotation that vanished between
    /// the source and the model would make a regenerated declaration
    /// differ from the one that was written.
    pub(crate) fn apply_use_site(&mut self, written: &[Applied], target: AnyId, site: UseSite) {
        for applied in written {
            // `@key` on a union discriminator is a use site the clause
            // permits: Table 21 lists `@key` against "Structure Members,
            // Union discriminator" and §7.2.2.4.4.4.8's own example is
            // `union MyCommand switch (@key CommandKind)`. It is carried
            // by default (`KeyOnUnionDiscriminator::Carried`): it reaches
            // `XTypesFacet::discriminator_is_key`, and from there
            // `UnionDiscriminatorFlag::IS_KEY` in the discriminator's
            // `TypeObject` flags, §7.2.4.4.4.5's assignability rule
            // ("Either the discriminators of both T1 and T2 are keys or
            // neither are keys") and a KeyHash over the discriminator.
            //
            // The measured toolchains refuse the declaration (Cyclone's
            // `idlc`, `rtiddsgen`) or generate no topic type for a union
            // (`fastddsgen`). `KeyOnUnionDiscriminator::Refused` is the
            // setting that reports it instead, with the clause named, so
            // the diagnostic does not claim the specification is on its
            // side.
            if applied.name == "key" && site == UseSite::Discriminator {
                if self.options.key_on_union_discriminator
                    == crate::KeyOnUnionDiscriminator::Carried
                {
                    self.set_xtypes(target, |x| x.discriminator_is_key = true);
                    continue;
                }
                self.annotation_error(
                    applied,
                    "`@key` on a union discriminator is not carried under \
                     `KeyOnUnionDiscriminator::Refused`: the clause permits it and no measured \
                     implementation accepts it, so this setting reports it rather than \
                     computing a KeyHash no peer computes",
                    "DDS-XTypes 1.3 §7.2.2.4.4.4.8, Table 21",
                );
                continue;
            }
            // `@external` on the elements of a collection (§7.3.1.2.1.4):
            // `sequence<@external Foo, 42>`, `map<string, @external Foo,
            // 42>`. It is the collection's `element_external` property,
            // and the `IS_EXTERNAL` bit of its identifier's element
            // flags. Table 21 lists `@external` for "Structure Members,
            // Union members (except union discriminator)", so the
            // discriminator keeps the refusal below.
            if applied.name == "external" && matches!(site, UseSite::Element | UseSite::MapKey) {
                let value = self.flag_argument(applied);
                self.set_xtypes(target, |x| match site {
                    UseSite::MapKey => x.key_is_external = value,
                    _ => x.element_is_external = value,
                });
                continue;
            }
            // A user-declared annotation is kept whole where the model
            // has a place for it: DDS-XTypes 1.3 §7.2.2.6 attaches
            // annotations to "an element/member/literal of an
            // aggregated/collection/enumerated type", and §7.3.1.2.5
            // applies them to the implicit discriminator member "by
            // applying them to the discriminator type declaration in the
            // header of the union type's definition". They reach the
            // `ann_custom` of the collection's element and of the
            // union's discriminator.
            if !zerodds_ast::node::annotation::BUILTIN_ANNOTATIONS.contains(&applied.name.as_str())
            {
                let annotation = self.user_annotation(applied);
                self.set_xtypes(target, |x| match site {
                    UseSite::Element => x.element_annotations.push(annotation),
                    UseSite::MapKey => x.key_annotations.push(annotation),
                    UseSite::Discriminator => x.discriminator_annotations.push(annotation),
                });
                continue;
            }
            if applied.name != "try_construct" {
                self.annotation_error(
                    applied,
                    &alloc::format!(
                        "`@{}` has no meaning on {}; only `@try_construct` and a user-declared \
                         annotation do (and `@external` on a collection element, `@key` on a \
                         discriminator)",
                        applied.name,
                        site.describe()
                    ),
                    "DDS-XTypes 1.3 §7.3.1.2.1.12",
                );
                continue;
            }
            let Some(behaviour) = self.try_construct_behaviour(applied) else {
                continue;
            };
            // Table 17: "If a key element of O2 cannot construct the
            // corresponding key type of T1 the entire map O2 cannot
            // construct any object of type T1" — with no TryConstruct
            // escape, which contradicts §7.2.2.7's sentence that a map
            // "may explicitly declare that their key ... has one of
            // three kinds". `opendds_idl` resolves it the same way and
            // aborts: "map keys may only use DISCARD as their
            // @try_construct mode".
            if site == UseSite::MapKey && behaviour != TryConstruct::Discard {
                self.annotation_error(
                    applied,
                    "a map key can only be DISCARD: a key that cannot be constructed \
                     fails the whole map",
                    "DDS-XTypes 1.3 Table 17",
                );
                continue;
            }
            // Table 19 admits only DEFAULT for a discriminator, and a
            // discriminator is never a string, sequence or map, so TRIM
            // is Table 11's unspecified case as well. Cyclone refuses
            // TRIM on any unbounded type outright; refusing it here
            // keeps a declaration from claiming a behaviour no reader
            // can give it.
            if site == UseSite::Discriminator && behaviour == TryConstruct::Trim {
                self.annotation_error(
                    applied,
                    "a union discriminator cannot be TRIM: it is not a string, \
                     wstring, sequence or map",
                    "DDS-XTypes 1.3 Table 11, Table 19",
                );
                continue;
            }
            self.set_xtypes(target, |x| match site {
                UseSite::Element => x.element_try_construct = Some(behaviour),
                UseSite::MapKey => x.key_try_construct = Some(behaviour),
                UseSite::Discriminator => x.discriminator_try_construct = Some(behaviour),
            });
        }
    }

    fn extensibility_argument(&mut self, applied: &Applied, target: AnyId) {
        let extensibility = match applied.arguments.first().and_then(Argument::identifier) {
            Some("FINAL") => Extensibility::Final,
            Some("APPENDABLE") => Extensibility::Appendable,
            Some("MUTABLE") => Extensibility::Mutable,
            other => {
                self.annotation_error(
                    applied,
                    &alloc::format!(
                        "`@extensibility` takes FINAL, APPENDABLE or MUTABLE, not `{}`",
                        other.unwrap_or("nothing")
                    ),
                    "DDS-XTypes 1.3 §7.2.4.4",
                );
                return;
            }
        };
        self.set_extensibility(target, extensibility);
    }

    fn range_annotation(&mut self, applied: &Applied, target: AnyId) {
        let mut min = None;
        let mut max = None;
        match applied.name.as_str() {
            "min" => min = applied.arguments.first().and_then(|a| a.value),
            "max" => max = applied.arguments.first().and_then(|a| a.value),
            _ => {
                for argument in &applied.arguments {
                    match argument.name.as_deref() {
                        Some("min") => min = argument.value,
                        Some("max") => max = argument.value,
                        // The positional form is `@range(min, max)`.
                        None if min.is_none() => min = argument.value,
                        None => max = argument.value,
                        Some(other) => {
                            let message =
                                alloc::format!("`@range` has no parameter named `{other}`");
                            self.annotation_error(applied, &message, "DDS-XTypes 1.3 Annex B");
                        }
                    }
                }
            }
        }
        self.set_xtypes(target, |x| {
            let existing = x.range.unwrap_or(RangeConstraint {
                min: None,
                max: None,
            });
            x.range = Some(RangeConstraint {
                min: min.or(existing.min),
                max: max.or(existing.max),
            });
        });
    }

    fn verbatim_annotation(&mut self, applied: &Applied, target: AnyId) {
        let mut language = String::from("*");
        let mut placement = VerbatimPlacement::BeforeDeclaration;
        let mut text = String::new();
        for argument in &applied.arguments {
            match argument.name.as_deref() {
                Some("language") => {
                    if let Some(value) = argument.text.clone() {
                        language = value;
                    }
                }
                Some("placement") => {
                    let written = argument
                        .text
                        .clone()
                        .or_else(|| argument.identifier().map(ToString::to_string));
                    match written.as_deref().and_then(verbatim_placement) {
                        Some(found) => placement = found,
                        None => self.annotation_error(
                            applied,
                            "`@verbatim` placement must be one of the six IDL 4.2 §8.3.5.1 names",
                            "OMG IDL 4.2 §8.3.5.1",
                        ),
                    }
                }
                Some("text") | None => {
                    if let Some(value) = argument.text.clone() {
                        text = value;
                    }
                }
                Some(other) => {
                    let message = alloc::format!("`@verbatim` has no parameter named `{other}`");
                    self.annotation_error(applied, &message, "OMG IDL 4.2 §8.3.5.1");
                }
            }
        }
        self.set_xtypes(target, |x| {
            x.verbatim.push(VerbatimDirective {
                language: language.clone(),
                placement,
                text: text.clone(),
            });
        });
    }

    fn topic_annotation(&mut self, applied: &Applied, target: AnyId) {
        let mut topic = TopicAnnotation::default();
        for argument in &applied.arguments {
            match argument.name.as_deref() {
                Some("name") | None => topic.name = argument.text.clone(),
                Some("platform") => topic.platform = argument.text.clone(),
                Some(other) => {
                    let message = alloc::format!("`@topic` has no parameter named `{other}`");
                    self.annotation_error(applied, &message, "DDS-XTypes 1.3 Annex B");
                }
            }
        }
        self.set_xtypes(target, |x| x.topic = Some(topic.clone()));
    }

    /// `@data_representation(XCDR1 | XCDR2)`.
    ///
    /// A mask, not one value: the argument is a bitwise or of the
    /// representations a type allows. DDS-XTypes 1.3 §7.6.3.1.1.
    fn data_representation_annotation(&mut self, applied: &Applied, target: AnyId) {
        let mut mask = DataRepresentationMask::default();
        let mut unknown: Option<String> = None;
        for argument in &applied.arguments {
            for identifier in &argument.identifiers {
                match identifier.as_str() {
                    "XCDR1" | "XCDR" => mask.xcdr1 = true,
                    "XCDR2" => mask.xcdr2 = true,
                    "XML" => mask.xml = true,
                    other => unknown = Some(other.to_string()),
                }
            }
        }
        if let Some(other) = unknown {
            let message =
                alloc::format!("`@data_representation` has no representation named `{other}`");
            self.annotation_error(applied, &message, "DDS-XTypes 1.3 §7.6.3.1.1");
            return;
        }
        if mask.is_empty() {
            self.annotation_error(
                applied,
                "`@data_representation` must allow at least one representation",
                "DDS-XTypes 1.3 §7.6.3.1.1",
            );
            return;
        }
        self.set_xtypes(target, |x| x.data_representation = Some(mask));
    }

    fn set_oneway(&mut self, target: AnyId) {
        if let AnyId::Operation(id, _) = target {
            let _ = self.ast.set_oneway(id, true);
        }
    }

    // -- helpers ---------------------------------------------------------

    fn set_extensibility(&mut self, target: AnyId, extensibility: Extensibility) {
        self.set_xtypes(target, |x| x.extensibility = Some(extensibility));
    }

    fn set_xtypes(&mut self, target: AnyId, edit: impl FnOnce(&mut zerodds_ast::XTypesFacet)) {
        if let Some(facets) = self.ast.facets_mut(target) {
            edit(facets.xtypes_mut());
        }
    }

    fn set_corba(&mut self, target: AnyId, edit: impl FnOnce(&mut zerodds_ast::CorbaFacet)) {
        if let Some(facets) = self.ast.facets_mut(target) {
            edit(facets.corba_mut());
        }
    }

    /// The integral value of an annotation's first argument.
    fn integral_argument(&mut self, applied: &Applied) -> Option<i128> {
        let argument = applied.arguments.first()?;
        let expr = argument.value?;
        match self.ast.eval_integral(expr) {
            Ok(value) => Some(value),
            Err(_) => {
                let message = alloc::format!("`@{}` needs an integer constant", applied.name);
                self.diagnose_at(
                    DiagnosticCode::InvalidAnnotation,
                    argument.span,
                    &message,
                    "OMG IDL 4.2 §8.2",
                );
                None
            }
        }
    }

    /// A boolean-valued annotation, which defaults to true when written
    /// without an argument: `@nested` means `@nested(TRUE)`.
    ///
    /// An argument that is not a constant boolean is refused: reading it
    /// as TRUE would turn `@key(SOME_NAME_THAT_DOES_NOT_EVALUATE)` into
    /// `@key`.
    fn flag_argument(&mut self, applied: &Applied) -> bool {
        let Some(argument) = applied.arguments.first() else {
            return true;
        };
        let Some(expr) = argument.value else {
            // An argument that was written and has no value the model
            // holds is not `@key` with its default.
            let message = alloc::format!("`@{}` takes TRUE or FALSE", applied.name);
            self.diagnose_at(
                DiagnosticCode::InvalidAnnotation,
                argument.span,
                &message,
                "DDS-XTypes 1.3 §7.3.1.2.1",
            );
            return true;
        };
        match self.ast.eval_integral(expr) {
            Ok(value) => value != 0,
            Err(_) => {
                let message = alloc::format!("`@{}` takes TRUE or FALSE", applied.name);
                self.diagnose_at(
                    DiagnosticCode::InvalidAnnotation,
                    argument.span,
                    &message,
                    "DDS-XTypes 1.3 §7.3.1.2.1",
                );
                true
            }
        }
    }

    fn name_of(&self, target: AnyId) -> Option<String> {
        Some(match target {
            AnyId::Member(id, _) => self.ast.member(id).ok()?.name.clone(),
            AnyId::Type(id, _) => self.ast.type_(id).ok()?.name.clone(),
            AnyId::Enumerator(id, _) => self.ast.enumerator(id).ok()?.name.clone(),
            AnyId::BitValue(id, _) => self.ast.bit_value(id).ok()?.name.clone(),
            _ => return None,
        })
    }

    pub(crate) fn annotation_error(
        &mut self,
        applied: &Applied,
        message: &str,
        spec: &'static str,
    ) {
        self.diagnose_at(
            DiagnosticCode::InvalidAnnotation,
            applied.span,
            message,
            spec,
        );
    }
}

/// The `@verbatim` placement a name stands for.
///
/// IDL 4.2 §8.3.5.1 spells the six as an enumeration (`BEGIN_FILE`,
/// `BEFORE_DECLARATION`, `BEGIN_DECLARATION`, `END_DECLARATION`,
/// `AFTER_DECLARATION`, `END_FILE`); DDS-XTypes 1.3 §7.2.2.4.8.2 spells
/// them as strings (`begin-declaration-file`, `before-declaration`,
/// `begin-declaration`, `end-declaration`, `after-declaration`,
/// `end-declaration-file`) and says "It shall be interpreted in a
/// case-insensitive manner". Both spellings are read, in any case.
#[must_use]
pub fn verbatim_placement(written: &str) -> Option<VerbatimPlacement> {
    let normalised = written.to_ascii_lowercase().replace('_', "-");
    Some(match normalised.as_str() {
        "begin-file" | "begin-declaration-file" => VerbatimPlacement::BeginFile,
        "before-declaration" => VerbatimPlacement::BeforeDeclaration,
        "begin-declaration" => VerbatimPlacement::BeginDeclaration,
        "end-declaration" => VerbatimPlacement::EndDeclaration,
        "after-declaration" => VerbatimPlacement::AfterDeclaration,
        "end-file" | "end-declaration-file" => VerbatimPlacement::EndFile,
        _ => return None,
    })
}
